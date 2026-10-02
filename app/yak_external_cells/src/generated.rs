/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The files of a cell that an origin generates from a package manager's resolution, as the
//! `cargo` and `go` origins do.
//!
//! The build files exist only in memory. Each third-party package is a package of the cell,
//! whose sources are the files that the package manager downloaded for it. The cell copies a
//! package's sources into `yak-out` the first time they are read and declares them to the
//! materializer.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use allocative::Allocative;
use cmp_any::PartialEqAny;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::EqualityBehavior;
use dice::Key;
use dice::NoValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use yak_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
use yak_common::cas_digest::CasDigestConfig;
use yak_common::dice::data::HasIoProvider;
use yak_common::file_ops::delegate::FileOpsDelegate;
use yak_common::file_ops::metadata::FileDigestConfig;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::FileType;
use yak_common::file_ops::metadata::RawDirEntry;
use yak_common::file_ops::metadata::RawPathMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_common::io::IoProvider;
use yak_common::io::fs::FsIoProvider;
use yak_common::legacy_configs::dice::HasLegacyConfigs;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePath;
use yak_core::fs::project::ProjectRoot;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::fs::yak_out_path::YakOutPathResolver;
use yak_directory::directory::directory::Directory;
use yak_error::YakErrorContext;
use yak_error::internal_error;
use yak_execute::artifact_value::ArtifactValue;
use yak_execute::digest_config::HasDigestConfig;
use yak_execute::directory::INTERNER;
use yak_execute::entry::build_entry_from_disk;
use yak_execute::execute::blocking::HasBlockingExecutor;
use yak_execute::execute::blocking::IoRequest;
use yak_execute::execute::clean_output_paths::CleanOutputPaths;
use yak_execute::materialize::materializer::DeclareArtifactPayload;
use yak_execute::materialize::materializer::HasMaterializer;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_path::AbsPath;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;

/// The name of the cell's build files. A cell without a `.yakconfig` uses the default build file
/// name.
pub(crate) const BUILD_FILE: &str = "YAK";

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum GeneratedCellError {
    #[error("`{command}` failed with {status}:\n{stderr}")]
    CommandFailed {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("Could not run `{command}`, which the {origin} cell needs: {error}")]
    CommandNotRun {
        origin: &'static str,
        command: String,
        error: String,
    },
    #[error("Expected the sources of `{0}` in `yak-out` after copying them")]
    NoSources(String),
}

#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
pub(crate) struct GeneratedFile {
    contents: Arc<str>,
    metadata: FileMetadata,
}

impl GeneratedFile {
    pub(crate) fn new(contents: String, digest_config: CasDigestConfig) -> GeneratedFile {
        GeneratedFile {
            metadata: FileMetadata {
                digest: TrackedFileDigest::from_content(contents.as_bytes(), digest_config),
                is_executable: false,
            },
            contents: Arc::from(contents),
        }
    }
}

/// A third-party package, whose directory in the cell holds its generated build file and the
/// sources that the package manager downloaded.
#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
pub(crate) struct GeneratedPackage {
    /// The package's directory in the cell. It can have several components, but no package's
    /// directory is inside another's.
    pub(crate) dir: Arc<str>,
    /// The absolute path of the directory that the package manager put the sources in.
    pub(crate) source_dir: Arc<str>,
    pub(crate) build_file: GeneratedFile,
}

/// The generated contents of a cell.
#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
pub(crate) struct GeneratedCellContents {
    /// The files in the cell's root directory, such as its build file. Sorted by name.
    pub(crate) root_files: Vec<(Arc<str>, GeneratedFile)>,
    /// Sorted by directory.
    pub(crate) packages: Vec<GeneratedPackage>,
}

/// A path in a generated cell.
enum GeneratedPath<'a> {
    /// A directory that holds no file of the cell, only the directories of packages, such as
    /// the root or `golang.org/x` in a go cell.
    Parent(&'a CellRelativePath),
    File(&'a GeneratedFile),
    /// A path inside the directory of a third-party package. The path is empty for the
    /// directory itself.
    InPackage(&'a GeneratedPackage, &'a ForwardRelativePath),
    Missing,
}

/// GeneratedFileOpsDelegate serves the files of a generated cell. The build files come from
/// memory and the sources of third-party packages from their copies in `yak-out`.
#[derive(Allocative, Pagable)]
pub(crate) struct GeneratedFileOpsDelegate {
    cell: CellName,
    origin: ExternalCellOrigin,
    contents: GeneratedCellContents,
    yak_out_resolver: YakOutPathResolver,
    // The package sources are in yak-out but are read as source files, as the `git` origin
    // reads its checkout.
    io: FsIoProvider,
}

impl GeneratedFileOpsDelegate {
    pub(crate) async fn new(
        ctx: &mut DiceComputations<'_>,
        cell: CellName,
        origin: ExternalCellOrigin,
        contents: GeneratedCellContents,
    ) -> yak_error::Result<GeneratedFileOpsDelegate> {
        let artifact_fs = ctx.get_artifact_fs().await?;
        Ok(GeneratedFileOpsDelegate {
            cell,
            origin,
            contents,
            yak_out_resolver: artifact_fs.yak_out_path_resolver().clone(),
            io: FsIoProvider::new(
                artifact_fs.fs().dupe(),
                ctx.global_data().get_digest_config().cas_digest_config(),
            ),
        })
    }

    /// Whether two delegates serve the same files, so that an edit that leaves the generated
    /// files unchanged does not invalidate the packages of the cell.
    pub(crate) fn same_contents(&self, other: &GeneratedFileOpsDelegate) -> bool {
        self.contents == other.contents
    }

    fn package(&self, dir: &str) -> Option<&GeneratedPackage> {
        self.contents
            .packages
            .binary_search_by(|p| (*p.dir).cmp(dir))
            .ok()
            .map(|i| &self.contents.packages[i])
    }

    /// The directories of packages below the directory `prefix`, which ends with `/` or is
    /// empty.
    fn packages_under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        let start = self.contents.packages.partition_point(|p| *p.dir < *prefix);
        self.contents.packages[start..]
            .iter()
            .map(|p| &*p.dir)
            .take_while(move |dir| dir.starts_with(prefix))
    }

    fn lookup<'a>(&'a self, path: &'a CellRelativePath) -> GeneratedPath<'a> {
        if path.is_empty() {
            return GeneratedPath::Parent(path);
        }
        if let Ok(i) = self
            .contents
            .root_files
            .binary_search_by(|(name, _)| (**name).cmp(path.as_str()))
        {
            return GeneratedPath::File(&self.contents.root_files[i].1);
        }
        // The package whose directory is `path` or a directory above it.
        let mut prefix_len = 0;
        for component in path.iter() {
            prefix_len += if prefix_len == 0 {
                component.as_str().len()
            } else {
                component.as_str().len() + 1
            };
            let dir = &path.as_str()[..prefix_len];
            if let Some(package) = self.package(dir) {
                let rest =
                    ForwardRelativePath::new(path.as_str()[prefix_len..].trim_start_matches('/'))
                        .expect("the rest of a forward relative path is one");
                return if rest.as_str() == BUILD_FILE {
                    GeneratedPath::File(&package.build_file)
                } else {
                    GeneratedPath::InPackage(package, rest)
                };
            }
        }
        if self
            .packages_under(&format!("{}/", path.as_str()))
            .next()
            .is_some()
        {
            GeneratedPath::Parent(path)
        } else {
            GeneratedPath::Missing
        }
    }

    fn resolve(&self, path: &CellRelativePath) -> ProjectRelativePathBuf {
        self.yak_out_resolver
            .resolve_external_cell_source(path, self.origin.dupe())
    }

    /// The project path of the cell path `package/rest`, after copying the package's sources
    /// into it.
    async fn package_path(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: &GeneratedPackage,
        rest: &ForwardRelativePath,
    ) -> yak_error::Result<ProjectRelativePathBuf> {
        let dir = CellRelativePath::unchecked_new(&package.dir);
        copy_package_sources(ctx, self.resolve(dir), package.source_dir.dupe()).await?;
        Ok(self.resolve(&dir.join(rest)))
    }
}

#[pagable_typetag]
#[async_trait::async_trait]
impl FileOpsDelegate for GeneratedFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<String>> {
        match self.lookup(path) {
            GeneratedPath::File(file) => Ok(Some(file.contents.to_string())),
            GeneratedPath::InPackage(package, rest) if !rest.is_empty() => {
                let project_path = self.package_path(ctx, package, rest).await?;
                (&self.io as &dyn IoProvider)
                    .read_file_if_exists(project_path)
                    .await
            }
            GeneratedPath::Parent(_) | GeneratedPath::InPackage(..) | GeneratedPath::Missing => {
                Ok(None)
            }
        }
    }

    async fn read_dir(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Arc<[RawDirEntry]>> {
        let mut entries =
            match self.lookup(path) {
                GeneratedPath::Parent(dir) => {
                    let mut entries: Vec<RawDirEntry> = Vec::new();
                    if dir.is_empty() {
                        entries.extend(self.contents.root_files.iter().map(|(name, _)| {
                            RawDirEntry {
                                file_name: name.as_ref().into(),
                                file_type: FileType::File,
                            }
                        }));
                    }
                    let prefix = if dir.is_empty() {
                        String::new()
                    } else {
                        format!("{}/", dir.as_str())
                    };
                    let mut children: Vec<&str> = self
                        .packages_under(&prefix)
                        .map(|p| p[prefix.len()..].split('/').next().unwrap_or_default())
                        .collect();
                    children.dedup();
                    entries.extend(children.into_iter().map(|name| RawDirEntry {
                        file_name: name.into(),
                        file_type: FileType::Directory,
                    }));
                    entries
                }
                GeneratedPath::InPackage(package, rest) => {
                    let project_path = self.package_path(ctx, package, rest).await?;
                    let mut entries = (&self.io as &dyn IoProvider)
                        .read_dir(project_path)
                        .await
                        .with_yak_error_context(|| format!("Error listing dir `{path}`"))?;
                    if rest.is_empty() {
                        entries.push(RawDirEntry {
                            file_name: BUILD_FILE.into(),
                            file_type: FileType::File,
                        });
                    }
                    entries
                }
                GeneratedPath::File(_) | GeneratedPath::Missing => {
                    return Err(yak_error::yak_error!(
                        yak_error::ErrorTag::Input,
                        "The cell `{}` has no directory `{path}`",
                        self.cell
                    ));
                }
            };
        // Make sure entries are deterministic, since read_dir isn't.
        entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(entries.into())
    }

    async fn read_path_metadata_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<RawPathMetadata>> {
        let (package, rest) = match self.lookup(path) {
            GeneratedPath::Parent(_) => return Ok(Some(RawPathMetadata::Directory)),
            GeneratedPath::File(file) => {
                return Ok(Some(RawPathMetadata::File(file.metadata.clone())));
            }
            GeneratedPath::Missing => return Ok(None),
            GeneratedPath::InPackage(_, rest) if rest.is_empty() => {
                return Ok(Some(RawPathMetadata::Directory));
            }
            GeneratedPath::InPackage(package, rest) => (package, rest),
        };
        let project_path = self.package_path(ctx, package, rest).await?;
        let Some(metadata) = (&self.io as &dyn IoProvider)
            .read_path_metadata_if_exists(project_path)
            .await
            .with_yak_error_context(|| format!("Error accessing metadata for path `{path}`"))?
        else {
            return Ok(None);
        };
        let base = self.resolve(CellRelativePath::empty());
        Ok(Some(metadata.try_map(
            |target| match target.strip_prefix_opt(&base) {
                Some(target) => Ok(Arc::new(CellPath::new(self.cell, target.to_owned().into()))),
                None => Err(internal_error!(
                    "Non-cell internal symlink at `{}` in cell `{}`",
                    target,
                    self.cell
                )),
            },
        )?))
    }

    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::new(&self.contents)
    }
}

/// Copies the sources of a third-party package from the directory that the package manager put
/// them in.
struct CopyPackageSources {
    from: Arc<str>,
    to: ProjectRelativePathBuf,
}

/// Copies the directory `from` to `to`, following symbolic links. It leaves out Git's metadata,
/// the marker file that Cargo writes after unpacking a package, and build files, which would
/// split the package. The copies of files are writable, since Go's module cache holds
/// read-only files.
fn copy_dir(from: &AbsPath, to: &AbsPath, top: bool) -> yak_error::Result<()> {
    fs_util::create_dir_all(to)?;
    for entry in std::fs::read_dir(from.as_path())
        .with_yak_error_context(|| format!("Error listing `{}`", from.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            // yak ignores file names that are not UTF-8.
            continue;
        };
        if matches!(name, ".git" | BUILD_FILE) || (top && name == ".cargo-ok") {
            continue;
        }
        let (from, to) = (from.join(name), to.join(name));
        let Ok(metadata) = fs_util::metadata(&from) else {
            // A broken symbolic link.
            continue;
        };
        if metadata.is_dir() {
            copy_dir(&from, &to, false)?;
        } else {
            fs_util::copy(&from, &to).categorize_internal()?;
            let mut permissions = fs_util::metadata(&to).categorize_internal()?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            if permissions.readonly() {
                permissions.set_readonly(false);
                std::fs::set_permissions(to.as_path(), permissions).with_yak_error_context(
                    || format!("Error making `{}` writable", to.display()),
                )?;
            }
        }
    }
    Ok(())
}

impl IoRequest for CopyPackageSources {
    fn execute(self: Box<Self>, project_fs: &ProjectRoot) -> yak_error::Result<()> {
        copy_dir(
            AbsPath::new(Path::new(&*self.from))?,
            &project_fs.resolve(&self.to),
            true,
        )
    }
}

/// Copies the sources of the package in `source_dir` to `path`, unless an earlier command
/// declared them there already. The path identifies the package's sources, because it names the
/// package's version.
async fn copy_package_sources(
    ctx: &mut DiceComputations<'_>,
    path: ProjectRelativePathBuf,
    source_dir: Arc<str>,
) -> yak_error::Result<()> {
    #[derive(
        Clone,
        Dupe,
        Debug,
        derive_more::Display,
        PartialEq,
        Eq,
        Hash,
        Allocative,
        Pagable
    )]
    #[display("({}, {})", _0, _1)]
    #[pagable_typetag(dice::DiceKeyDyn)]
    struct PackageSourcesKey(Arc<ProjectRelativePathBuf>, Arc<str>);

    #[async_trait::async_trait]
    impl Key for PackageSourcesKey {
        type Value = yak_error::Result<()>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            cancellations: &CancellationContext,
        ) -> Self::Value {
            let path: &ProjectRelativePath = &self.0;
            // Commands at different DICE versions can compute this key at once, so each path
            // has a lock.
            static LOCKS: LazyLock<
                Mutex<HashMap<ProjectRelativePathBuf, Arc<tokio::sync::Mutex<()>>>>,
            > = LazyLock::new(Default::default);
            let lock = LOCKS
                .lock()
                .unwrap()
                .entry(path.to_owned())
                .or_default()
                .dupe();
            let _guard = lock.lock().await;

            let materializer = ctx.per_transaction_data().get_materializer();
            if materializer.has_artifact_at(path.to_owned()).await? {
                return Ok(());
            }
            cancellations
                .critical_section(|| async {
                    let io = ctx.get_blocking_executor();
                    // Held until the copy is reported below.
                    let output_lease = materializer.prepare_outputs(vec![path.to_owned()]).await?;
                    io.execute_io(
                        Box::new(CleanOutputPaths {
                            paths: vec![path.to_owned()],
                        }),
                        cancellations,
                    )
                    .await?;
                    io.execute_io(
                        Box::new(CopyPackageSources {
                            from: self.1.dupe(),
                            to: path.to_owned(),
                        }),
                        cancellations,
                    )
                    .await?;

                    // The materializer needs the digests of the copy.
                    let proj_root = ctx.global_data().get_io_provider().project_root().root();
                    let digest_config = ctx.global_data().get_digest_config();
                    let file_digest_config =
                        FileDigestConfig::build(digest_config.cas_digest_config());
                    let entry = build_entry_from_disk(
                        proj_root.join(path),
                        file_digest_config,
                        io,
                        proj_root,
                    )
                    .await?
                    .0
                    .ok_or_else(|| GeneratedCellError::NoSources(self.1.to_string()))?;
                    let entry = entry.map_dir(|d| {
                        d.to_builder()
                            .fingerprint(digest_config.as_directory_serializer())
                            .shared(&*INTERNER)
                    });
                    materializer
                        .declare_existing(
                            &output_lease,
                            vec![DeclareArtifactPayload {
                                path: path.to_owned(),
                                artifact: ArtifactValue::new(entry, None),
                            }],
                        )
                        .await
                })
                .await
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            EqualityBehavior::Compare(|x, y| x.is_ok() && y.is_ok())
        }

        fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
            // The value carries nothing, and the materializer remembers the copy.
            NoValueSerialize::<Self::Value>::new()
        }
    }

    ctx.compute(&PackageSourcesKey(Arc::new(path), source_dir))
        .await?
        .clone()
}

/// tool_identity reads `tool_identity.<tool>` of the root cell through DICE, so that a computation
/// that runs `tool` runs again when the daemon finds another version of it.
pub(crate) async fn tool_identity<'d>(
    ctx: &mut DiceComputations<'d>,
    tool: &str,
) -> yak_error::Result<Option<Arc<str>>> {
    ctx.get_legacy_root_config_on_dice().await?.lookup(
        ctx,
        YakconfigKeyRef {
            section: "tool_identity",
            property: tool,
        },
    )
}

/// Runs `program` with `args` and the environment variables `env` in `dir`, and returns its
/// standard output. `origin` names the cell's origin in the error when the program cannot run.
pub(crate) async fn run(
    origin: &'static str,
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
    dir: &AbsPath,
) -> yak_error::Result<String> {
    let command = format!("{program} {}", args.join(" "));
    let output = tokio::process::Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(dir.as_path())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| GeneratedCellError::CommandNotRun {
            origin,
            command: command.clone(),
            error: e.to_string(),
        })?;
    if !output.status.success() {
        let env: String = env.iter().map(|(k, v)| format!("{k}={v} ")).collect();
        return Err(GeneratedCellError::CommandFailed {
            command: format!("{env}{command}"),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
