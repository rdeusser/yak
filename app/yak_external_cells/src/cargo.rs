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
//! Each third-party package is a package of the cell named `<name>-<version>`, which
//! `generated.rs` serves. Cargo checked the package's sources against `Cargo.lock` when it
//! downloaded them, and `cargo metadata` follows the workspace's `.cargo/config.toml`, so
//! packages from private registries, replaced sources, and Git build as they do with
//! `cargo build`.

use std::path::Path;
use std::sync::Arc;

use allocative::Allocative;
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
use yak_common::file_ops::dice::DiceFileComputations;
use yak_core::cells::external::CargoCellSetup;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::name::CellName;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_execute::digest_config::HasDigestConfig;
use yak_external_cells_cargo::CargoPlatform;
use yak_external_cells_cargo::DEFAULT_PLATFORMS;
use yak_external_cells_cargo::cfg::TargetCfg;
use yak_external_cells_cargo::generate_third_party;
use yak_external_cells_cargo::generate_workspace;
use yak_external_cells_cargo::metadata::Metadata;
use yak_fs::paths::abs_path::AbsPath;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;

use crate::generated::BUILD_FILE;
use crate::generated::GeneratedCellContents;
use crate::generated::GeneratedFile;
use crate::generated::GeneratedFileOpsDelegate;
use crate::generated::GeneratedPackage;
use crate::generated::run;

/// The file whose `cargo_workspace_member` macro declares the targets of a workspace member.
const WORKSPACE_FILE: &str = "workspace.bzl";

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum CargoCellError {
    #[error("`{0}` has no `Cargo.lock` next to it. Run `cargo generate-lockfile`.")]
    MissingLockFile(String),
    #[error(
        "The cargo cell's manifest `{0}` does not exist. Set `manifest` in its `[external_cell_<name>]` section."
    )]
    MissingManifest(String),
}

/// The Cargo platforms of the prelude, with the cfg values that `rustc` reports for each.
async fn platforms(dir: &AbsPath) -> yak_error::Result<Vec<CargoPlatform>> {
    let outputs =
        futures::future::try_join_all(DEFAULT_PLATFORMS.iter().map(|(_, triple)| async move {
            run(
                "cargo",
                "rustc",
                &["--print", "cfg", "--target", triple],
                &[],
                dir,
            )
            .await
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
) -> yak_error::Result<GeneratedCellContents> {
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
            "cargo",
            &[
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--manifest-path",
                &abs_manifest.to_string(),
            ],
            &[],
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
    Ok(GeneratedCellContents {
        root_files: vec![
            (
                Arc::from(BUILD_FILE),
                GeneratedFile::new(third_party.root_build_file, digest_config),
            ),
            (
                Arc::from(WORKSPACE_FILE),
                GeneratedFile::new(workspace, digest_config),
            ),
        ],
        packages: third_party
            .packages
            .into_iter()
            .map(|p| GeneratedPackage {
                dir: Arc::from(p.dir),
                source_dir: Arc::from(p.source_dir),
                build_file: GeneratedFile::new(p.build_file, digest_config),
            })
            .collect(),
    })
}

pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: CargoCellSetup,
) -> yak_error::Result<Arc<GeneratedFileOpsDelegate>> {
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
        type Value = yak_error::Result<Arc<GeneratedFileOpsDelegate>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let contents = generate(ctx, self.0, &self.1).await?;
            Ok(Arc::new(
                GeneratedFileOpsDelegate::new(
                    ctx,
                    self.0,
                    ExternalCellOrigin::Cargo(self.1.dupe()),
                    contents,
                )
                .await?,
            ))
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            // An edit that leaves the generated files unchanged does not invalidate the
            // packages of the cell.
            EqualityBehavior::Compare(|x, y| match (x, y) {
                (Ok(x), Ok(y)) => x.same_contents(y),
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
