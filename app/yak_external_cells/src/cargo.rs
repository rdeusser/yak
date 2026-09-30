/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `cargo` external cell origin. Its cell holds the third-party packages of a Cargo
//! workspace, as `cargo metadata` resolves them, and the macro that declares the targets of the
//! workspace members.
//!
//! The build files exist only in memory. Each third-party package is a package of the cell,
//! named `<name>-<version>`, whose sources are the files that Cargo downloaded for it. The cell
//! copies a package's sources into `yak-out` the first time they are read and declares them to
//! the materializer. Cargo checked them against `Cargo.lock` when it downloaded them, and
//! `cargo metadata` follows the workspace's `.cargo/config.toml`, so packages from private
//! registries, replaced sources, and Git build as they do with `cargo build`.

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
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use yak_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
use yak_common::cas_digest::CasDigestConfig;
use yak_common::dice::cells::HasCellResolver;
use yak_common::dice::data::HasIoProvider;
use yak_common::file_ops::delegate::FileOpsDelegate;
use yak_common::file_ops::dice::DiceFileComputations;
use yak_common::file_ops::metadata::FileDigestConfig;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::FileType;
use yak_common::file_ops::metadata::RawDirEntry;
use yak_common::file_ops::metadata::RawPathMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_common::io::IoProvider;
use yak_common::io::fs::FsIoProvider;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::external::CargoCellSetup;
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
use yak_external_cells_cargo::CargoPlatform;
use yak_external_cells_cargo::DEFAULT_PLATFORMS;
use yak_external_cells_cargo::cfg::TargetCfg;
use yak_external_cells_cargo::generate_third_party;
use yak_external_cells_cargo::generate_workspace;
use yak_external_cells_cargo::metadata::Metadata;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_path::AbsPath;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;

/// The name of the cell's build files. A cell without a `.yakconfig` uses the default build file
/// name.
const BUILD_FILE: &str = "YAK";

/// The file whose `cargo_workspace_member` macro declares the targets of a workspace member.
const WORKSPACE_FILE: &str = "workspace.bzl";

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum CargoCellError {
    #[error("`{command}` failed with {status}:\n{stderr}")]
    CommandFailed {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("Could not run `{command}`, which the cargo cell needs: {error}")]
    CommandNotRun { command: String, error: String },
    #[error("`{0}` has no `Cargo.lock` next to it. Run `cargo generate-lockfile`.")]
    MissingLockFile(String),
    #[error(
        "The cargo cell's manifest `{0}` does not exist. Set `manifest` in its `[external_cell_<name>]` section."
    )]
    MissingManifest(String),
    #[error("Expected the sources of `{0}` in `yak-out` after copying them")]
    NoSources(String),
}

#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
struct CargoCellFile {
    contents: Arc<str>,
    metadata: FileMetadata,
}

impl CargoCellFile {
    fn new(contents: String, digest_config: CasDigestConfig) -> CargoCellFile {
        CargoCellFile {
            metadata: FileMetadata {
                digest: TrackedFileDigest::from_content(contents.as_bytes(), digest_config),
                is_executable: false,
            },
            contents: Arc::from(contents),
        }
    }
}

/// A third-party package, whose directory in the cell holds its generated build file and the
/// sources that Cargo downloaded.
#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
struct CargoCellPackage {
    /// `<name>-<version>`.
    dir: Arc<str>,
    /// The absolute path of the directory that Cargo put the sources in.
    source_dir: Arc<str>,
    build_file: CargoCellFile,
}

/// The generated contents of a cargo cell.
#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
struct CargoCellContents {
    build_file: CargoCellFile,
    workspace_file: CargoCellFile,
    /// Sorted by directory.
    packages: Vec<CargoCellPackage>,
}

/// A path in a cargo cell.
enum CargoCellPath<'a> {
    Root,
    File(&'a CargoCellFile),
    /// A path inside the directory of a third-party package. The path is empty for the
    /// directory itself.
    InPackage(&'a CargoCellPackage, &'a ForwardRelativePath),
    Missing,
}

/// CargoFileOpsDelegate serves the files of a cargo cell. The build files come from memory and
/// the sources of third-party packages from their copies in `yak-out`.
#[derive(Allocative, Pagable)]
pub(crate) struct CargoFileOpsDelegate {
    cell: CellName,
    setup: CargoCellSetup,
    contents: CargoCellContents,
    yak_out_resolver: YakOutPathResolver,
    // The package sources are in yak-out but are read as source files, as the `git` origin
    // reads its checkout.
    io: FsIoProvider,
}

impl CargoFileOpsDelegate {
    fn lookup<'a>(&'a self, path: &'a CellRelativePath) -> CargoCellPath<'a> {
        let mut components = path.iter();
        let Some(first) = components.next() else {
            return CargoCellPath::Root;
        };
        let rest = components.as_path();
        let root_file = match first.as_str() {
            BUILD_FILE => Some(&self.contents.build_file),
            WORKSPACE_FILE => Some(&self.contents.workspace_file),
            _ => None,
        };
        if let Some(file) = root_file {
            return if rest.is_empty() {
                CargoCellPath::File(file)
            } else {
                CargoCellPath::Missing
            };
        }
        match self
            .contents
            .packages
            .binary_search_by(|p| (*p.dir).cmp(first.as_str()))
        {
            Ok(i) => {
                let package = &self.contents.packages[i];
                if rest.as_str() == BUILD_FILE {
                    CargoCellPath::File(&package.build_file)
                } else {
                    CargoCellPath::InPackage(package, rest)
                }
            }
            Err(_) => CargoCellPath::Missing,
        }
    }

    fn resolve(&self, path: &CellRelativePath) -> ProjectRelativePathBuf {
        self.yak_out_resolver
            .resolve_external_cell_source(path, ExternalCellOrigin::Cargo(self.setup.dupe()))
    }

    /// The project path of the cell path `package/rest`, after copying the package's sources
    /// into it.
    async fn package_path(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: &CargoCellPackage,
        rest: &ForwardRelativePath,
    ) -> yak_error::Result<ProjectRelativePathBuf> {
        let dir = CellRelativePath::unchecked_new(&package.dir);
        copy_package_sources(ctx, self.resolve(dir), package.source_dir.dupe()).await?;
        Ok(self.resolve(&dir.join(rest)))
    }
}

#[pagable_typetag]
#[async_trait::async_trait]
impl FileOpsDelegate for CargoFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<String>> {
        match self.lookup(path) {
            CargoCellPath::File(file) => Ok(Some(file.contents.to_string())),
            CargoCellPath::InPackage(package, rest) if !rest.is_empty() => {
                let project_path = self.package_path(ctx, package, rest).await?;
                (&self.io as &dyn IoProvider)
                    .read_file_if_exists(project_path)
                    .await
            }
            CargoCellPath::Root | CargoCellPath::InPackage(..) | CargoCellPath::Missing => Ok(None),
        }
    }

    async fn read_dir(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Arc<[RawDirEntry]>> {
        let mut entries = match self.lookup(path) {
            CargoCellPath::Root => {
                let mut entries: Vec<RawDirEntry> = [BUILD_FILE, WORKSPACE_FILE]
                    .into_iter()
                    .map(|name| RawDirEntry {
                        file_name: name.into(),
                        file_type: FileType::File,
                    })
                    .collect();
                entries.extend(self.contents.packages.iter().map(|p| RawDirEntry {
                    file_name: p.dir.as_ref().into(),
                    file_type: FileType::Directory,
                }));
                entries
            }
            CargoCellPath::InPackage(package, rest) => {
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
            CargoCellPath::File(_) | CargoCellPath::Missing => {
                return Err(yak_error::yak_error!(
                    yak_error::ErrorTag::Input,
                    "The cargo cell `{}` has no directory `{path}`",
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
            CargoCellPath::Root => return Ok(Some(RawPathMetadata::Directory)),
            CargoCellPath::File(file) => {
                return Ok(Some(RawPathMetadata::File(file.metadata.clone())));
            }
            CargoCellPath::Missing => return Ok(None),
            CargoCellPath::InPackage(_, rest) if rest.is_empty() => {
                return Ok(Some(RawPathMetadata::Directory));
            }
            CargoCellPath::InPackage(package, rest) => (package, rest),
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

/// Copies the sources of a third-party package from the directory that Cargo put them in.
struct CopyPackageSources {
    from: Arc<str>,
    to: ProjectRelativePathBuf,
}

/// Copies the directory `from` to `to`, following symbolic links. It leaves out Git's metadata,
/// the marker file that Cargo writes after unpacking a package, and a build file at the top,
/// which the cell generates.
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
        if name == ".git" || (top && matches!(name, BUILD_FILE | ".cargo-ok")) {
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
/// declared them there already. A package's name and version identify its sources.
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
    struct CargoPackageSourcesKey(Arc<ProjectRelativePathBuf>, Arc<str>);

    #[async_trait::async_trait]
    impl Key for CargoPackageSourcesKey {
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
                    .ok_or_else(|| CargoCellError::NoSources(self.1.to_string()))?;
                    let entry = entry.map_dir(|d| {
                        d.to_builder()
                            .fingerprint(digest_config.as_directory_serializer())
                            .shared(&*INTERNER)
                    });
                    materializer
                        .declare_existing(vec![DeclareArtifactPayload {
                            path: path.to_owned(),
                            artifact: ArtifactValue::new(entry, None),
                        }])
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

    ctx.compute(&CargoPackageSourcesKey(Arc::new(path), source_dir))
        .await?
        .clone()
}

/// Runs `program` with `args` in `dir` and returns its standard output.
async fn run(program: &str, args: &[&str], dir: &AbsPath) -> yak_error::Result<String> {
    let command = format!("{program} {}", args.join(" "));
    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(dir.as_path())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| CargoCellError::CommandNotRun {
            command: command.clone(),
            error: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(CargoCellError::CommandFailed {
            command,
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// The Cargo platforms of the prelude, with the cfg values that `rustc` reports for each.
async fn platforms(dir: &AbsPath) -> yak_error::Result<Vec<CargoPlatform>> {
    let outputs =
        futures::future::try_join_all(DEFAULT_PLATFORMS.iter().map(|(_, triple)| async move {
            run("rustc", &["--print", "cfg", "--target", triple], dir).await
        }))
        .await?;
    DEFAULT_PLATFORMS
        .iter()
        .zip(outputs)
        .map(|((name, triple), output)| {
            Ok(CargoPlatform {
                name: (*name).to_owned(),
                triple: (*triple).to_owned(),
                cfg: TargetCfg::parse(&output)?,
            })
        })
        .collect()
}

async fn generate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: &CargoCellSetup,
) -> yak_error::Result<CargoCellContents> {
    let cells = ctx.get_cell_resolver().await?;
    let manifest = &*setup.manifest;
    let workspace_dir = manifest.parent().unwrap_or(ProjectRelativePath::empty());
    let lock_path = workspace_dir.join(ForwardRelativePath::new("Cargo.lock")?);

    // Read the manifests and the lock file through DICE, so that editing one of them computes
    // the cell again. Their contents come from `cargo metadata`.
    DiceFileComputations::read_file_if_exists(ctx, cells.get_cell_path(manifest).as_ref())
        .await?
        .ok_or_else(|| CargoCellError::MissingManifest(manifest.to_string()))?;
    DiceFileComputations::read_file_if_exists(ctx, cells.get_cell_path(&lock_path).as_ref())
        .await?
        .ok_or_else(|| CargoCellError::MissingLockFile(manifest.to_string()))?;

    let project_root = ctx.global_data().get_io_provider().project_root().dupe();
    let abs_dir = project_root.resolve(workspace_dir);
    let abs_manifest = project_root.resolve(manifest);
    let metadata = Metadata::parse(
        &run(
            "cargo",
            &[
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--manifest-path",
                &abs_manifest.to_string(),
            ],
            &abs_dir,
        )
        .await?,
    )?;

    for member in &metadata.workspace_members {
        let Some(package) = metadata.packages.iter().find(|p| &p.id == member) else {
            continue;
        };
        let Ok(member_manifest) =
            project_root.relativize_any(AbsPath::new(Path::new(&package.manifest_path))?)
        else {
            continue;
        };
        DiceFileComputations::read_file_if_exists(
            ctx,
            cells.get_cell_path(&member_manifest).as_ref(),
        )
        .await?;
    }

    let platforms = platforms(&abs_dir).await?;
    let third_party = generate_third_party(&metadata, &platforms)?;
    let workspace = generate_workspace(
        &metadata,
        &platforms,
        project_root.root().as_path(),
        cell_name.as_str(),
    )?;

    let digest_config = ctx
        .global_data()
        .get_digest_config()
        .cas_digest_config()
        .source_files_config();
    Ok(CargoCellContents {
        build_file: CargoCellFile::new(third_party.root_build_file, digest_config),
        workspace_file: CargoCellFile::new(workspace, digest_config),
        packages: third_party
            .packages
            .into_iter()
            .map(|p| CargoCellPackage {
                dir: Arc::from(p.dir),
                source_dir: Arc::from(p.source_dir),
                build_file: CargoCellFile::new(p.build_file, digest_config),
            })
            .collect(),
    })
}

pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: CargoCellSetup,
) -> yak_error::Result<Arc<CargoFileOpsDelegate>> {
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
    struct CargoFileOpsDelegateKey(CellName, CargoCellSetup);

    #[async_trait::async_trait]
    impl Key for CargoFileOpsDelegateKey {
        type Value = yak_error::Result<Arc<CargoFileOpsDelegate>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let contents = generate(ctx, self.0, &self.1).await?;
            let artifact_fs = ctx.get_artifact_fs().await?;
            Ok(Arc::new(CargoFileOpsDelegate {
                cell: self.0,
                setup: self.1.dupe(),
                contents,
                yak_out_resolver: artifact_fs.yak_out_path_resolver().clone(),
                io: FsIoProvider::new(
                    artifact_fs.fs().dupe(),
                    ctx.global_data().get_digest_config().cas_digest_config(),
                ),
            }))
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            // An edit that leaves the generated files unchanged does not invalidate the
            // packages of the cell.
            EqualityBehavior::Compare(|x, y| match (x, y) {
                (Ok(x), Ok(y)) => x.contents == y.contents,
                _ => false,
            })
        }

        fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
            OkPagableValueSerialize::<Self::Value>::new()
        }
    }

    ctx.compute(&CargoFileOpsDelegateKey(cell_name, setup))
        .await?
        .dupe()
}

pub(crate) async fn materialize_all(
    _ctx: &mut DiceComputations<'_>,
    cell: CellName,
    _setup: CargoCellSetup,
) -> yak_error::Result<ProjectRelativePathBuf> {
    Err(yak_error::yak_error!(
        yak_error::ErrorTag::Input,
        "The cargo cell `{cell}` is generated from `cargo metadata` and cannot be expanded"
    ))
}
