/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `cargo` external cell origin. Its cell holds a generated build file with a target for each
//! third-party package of a Cargo workspace, as `cargo metadata` resolves them. The files exist
//! only in memory. The third-party sources are downloaded by the `http_archive` targets of the
//! build file.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use allocative::Allocative;
use cmp_any::PartialEqAny;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::EqualityBehavior;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use yak_common::dice::cells::HasCellResolver;
use yak_common::dice::data::HasIoProvider;
use yak_common::file_ops::delegate::FileOpsDelegate;
use yak_common::file_ops::dice::DiceFileComputations;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::FileType;
use yak_common::file_ops::metadata::RawDirEntry;
use yak_common::file_ops::metadata::RawPathMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_core::cells::external::CargoCellSetup;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_execute::digest_config::HasDigestConfig;
use yak_external_cells_cargo::CargoPlatform;
use yak_external_cells_cargo::DEFAULT_PLATFORMS;
use yak_external_cells_cargo::cfg::TargetCfg;
use yak_external_cells_cargo::generate_third_party;
use yak_external_cells_cargo::generate_workspace;
use yak_external_cells_cargo::metadata::Checksums;
use yak_external_cells_cargo::metadata::Metadata;
use yak_fs::paths::abs_path::AbsPath;

/// The name of the cell's build file. A cell without a `.yakconfig` uses the default build file
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
}

#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
struct CargoCellFile {
    path: Arc<str>,
    contents: Arc<str>,
    metadata: FileMetadata,
}

/// CargoFileOpsDelegate serves the files of a cargo cell from memory. All of them are at the
/// root of the cell.
#[derive(Allocative, Pagable, PartialEq, Eq, Debug)]
pub(crate) struct CargoFileOpsDelegate {
    /// Sorted by path.
    files: Vec<CargoCellFile>,
}

impl CargoFileOpsDelegate {
    fn file(&self, path: &CellRelativePath) -> Option<&CargoCellFile> {
        self.files.iter().find(|f| *f.path == *path.as_str())
    }
}

#[pagable_typetag]
#[async_trait::async_trait]
impl FileOpsDelegate for CargoFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<String>> {
        Ok(self.file(path).map(|f| f.contents.to_string()))
    }

    /// Return the list of file outputs, sorted.
    async fn read_dir(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Arc<[RawDirEntry]>> {
        if !path.is_empty() {
            return Err(yak_error::yak_error!(
                yak_error::ErrorTag::Input,
                "The cargo cell has no directory `{path}`"
            ));
        }
        Ok(self
            .files
            .iter()
            .map(|f| RawDirEntry {
                file_name: f.path.as_ref().into(),
                file_type: FileType::File,
            })
            .collect())
    }

    async fn read_path_metadata_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<RawPathMetadata>> {
        if path.is_empty() {
            return Ok(Some(RawPathMetadata::Directory));
        }
        Ok(self
            .file(path)
            .map(|f| RawPathMetadata::File(f.metadata.clone())))
    }

    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::new(self)
    }
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
) -> yak_error::Result<CargoFileOpsDelegate> {
    let cells = ctx.get_cell_resolver().await?;
    let manifest = &*setup.manifest;
    let workspace_dir = manifest.parent().unwrap_or(ProjectRelativePath::empty());
    let lock_path = workspace_dir.join(yak_fs::paths::forward_rel_path::ForwardRelativePath::new(
        "Cargo.lock",
    )?);

    // Read the manifests and the lock file through DICE, so that editing one of them computes
    // the cell again. Their contents come from `cargo metadata`.
    DiceFileComputations::read_file_if_exists(ctx, cells.get_cell_path(manifest).as_ref())
        .await?
        .ok_or_else(|| CargoCellError::MissingManifest(manifest.to_string()))?;
    let lock =
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
    let build_file = generate_third_party(&metadata, &Checksums::parse(&lock)?, &platforms)?;
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
    let file = |path: &str, contents: String| CargoCellFile {
        path: Arc::from(path),
        metadata: FileMetadata {
            digest: TrackedFileDigest::from_content(contents.as_bytes(), digest_config),
            is_executable: false,
        },
        contents: Arc::from(contents),
    };
    Ok(CargoFileOpsDelegate {
        files: vec![
            file(BUILD_FILE, build_file),
            file(WORKSPACE_FILE, workspace),
        ],
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
            Ok(Arc::new(generate(ctx, self.0, &self.1).await?))
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            // An edit that leaves the generated files unchanged does not invalidate the
            // packages of the cell.
            EqualityBehavior::Compare(|x, y| match (x, y) {
                (Ok(x), Ok(y)) => x == y,
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
) -> yak_error::Result<yak_core::fs::project_rel_path::ProjectRelativePathBuf> {
    Err(yak_error::yak_error!(
        yak_error::ErrorTag::Input,
        "The cargo cell `{cell}` is generated from `cargo metadata` and cannot be expanded"
    ))
}
