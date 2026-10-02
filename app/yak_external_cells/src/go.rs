/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `go` external cell origin. Its cell holds the third-party packages of a Go module, as
//! `go list` resolves them, and the macros that declare the targets of the module's packages.
//!
//! Each third-party module version is a package of the cell, in a directory named
//! `<module path>@<version>` as in Go's module cache, which `generated.rs` serves. Go checked the
//! module's files against `go.sum` when it downloaded them.
//!
//! The cell runs `go list` again only when its inputs change: `go.mod`, `go.sum`, the names of
//! the module's files, the header of each Go file (`yak_external_cells_go::go_file_header`),
//! which build files below the module's root call `go_package()`, and the identity of `go`
//! (`tool_identity.go`). Editing a function body leaves them unchanged.

use std::collections::BTreeMap;
use std::sync::Arc;

use allocative::Allocative;
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
use yak_common::dice::cells::HasCellResolver;
use yak_common::dice::data::HasIoProvider;
use yak_common::file_ops::dice::DiceFileComputations;
use yak_common::file_ops::metadata::FileType;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::external::GoCellSetup;
use yak_core::cells::name::CellName;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_execute::digest_config::HasDigestConfig;
use yak_external_cells_go::ModuleInputs;
use yak_external_cells_go::PLATFORMS;
use yak_external_cells_go::generate as generate_cell;
use yak_external_cells_go::go_file_header;
use yak_external_cells_go::list::parse_list;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;

use crate::generated::BUILD_FILE;
use crate::generated::GeneratedCellContents;
use crate::generated::GeneratedFile;
use crate::generated::GeneratedFileOpsDelegate;
use crate::generated::GeneratedPackage;
use crate::generated::run;
use crate::generated::tool_identity;

/// The file whose `go_module` and `go_package` macros declare the targets of the module's
/// packages.
const MODULE_FILE: &str = "module.bzl";

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum GoCellError {
    #[error(
        "The go cell's module `{0}` does not exist. Set `module` in its `[external_cell_<name>]` section."
    )]
    MissingModule(String),
    #[error(
        "The Go module `{0}` vendors its dependencies in `vendor/`, which a go cell does not support. Remove the `vendor` directory."
    )]
    Vendored(String),
}

/// The inputs of the `go list` runs of a module.
#[derive(Allocative, PartialEq, Eq, Debug)]
struct GoModuleInputs {
    go_mod: String,
    go_sum: Option<String>,
    /// The files of the module outside nested modules, relative to the module's root, each with
    /// the header of a Go file. Sorted.
    files: Vec<(String, Option<Arc<str>>)>,
    /// The directories below the module's root, relative to it, that have a build file, each
    /// with whether that file calls `go_package()`.
    build_files: BTreeMap<String, bool>,
    /// The identity of `go` (`tool_identity.go`), whose version decides the standard library
    /// and what `go list` reports.
    go_identity: Option<Arc<str>>,
}

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
#[pagable_typetag(dice::DiceKeyDyn)]
struct GoFileHeaderKey(Arc<CellPath>);

#[async_trait::async_trait]
impl Key for GoFileHeaderKey {
    type Value = yak_error::Result<Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let source = DiceFileComputations::read_file_if_exists(ctx, self.0.as_ref().as_ref())
            .await?
            .unwrap_or_default();
        Ok(Arc::from(go_file_header(&source)))
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        })
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        NoValueSerialize::<Self::Value>::new()
    }
}

fn relative_join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// Reads the inputs of the module whose `go.mod` is `module` through DICE.
async fn read_inputs(
    ctx: &mut DiceComputations<'_>,
    module: &ProjectRelativePath,
) -> yak_error::Result<GoModuleInputs> {
    let cells = ctx.get_cell_resolver().await?;
    let module_dir = module.parent().unwrap_or(ProjectRelativePath::empty());
    let in_module = |rel: &str| -> yak_error::Result<CellPath> {
        Ok(cells.get_cell_path(&module_dir.join(ForwardRelativePath::new(rel)?)))
    };

    let go_mod =
        DiceFileComputations::read_file_if_exists(ctx, cells.get_cell_path(module).as_ref())
            .await?
            .ok_or_else(|| GoCellError::MissingModule(module.to_string()))?;
    let go_sum =
        DiceFileComputations::read_file_if_exists(ctx, in_module("go.sum")?.as_ref()).await?;
    if DiceFileComputations::read_path_metadata_if_exists(
        ctx,
        in_module("vendor/modules.txt")?.as_ref(),
    )
    .await?
    .is_some()
    {
        return Err(GoCellError::Vendored(module.to_string()).into());
    }
    let build_file_names =
        DiceFileComputations::buildfiles(ctx, cells.get_cell_path(module).cell())
            .await?
            .dupe();

    // List the module's directories one level at a time, leaving out nested modules, which
    // `go list ./...` leaves out too.
    let mut files: Vec<(String, Option<Arc<str>>)> = Vec::new();
    let mut go_files: Vec<(String, CellPath)> = Vec::new();
    let mut build_file_paths: Vec<(String, CellPath)> = Vec::new();
    let mut level = vec![String::new()];
    while !level.is_empty() {
        let listings = ctx
            .compute_join(level, async |ctx: &mut DiceComputations, rel: String| {
                let path = in_module(&rel)?;
                let listing = DiceFileComputations::read_dir(ctx, path.as_ref()).await?;
                yak_error::Ok((rel, path, listing.included.dupe()))
            })
            .await;
        level = Vec::new();
        for listing in listings {
            let (rel, dir_path, entries) = listing?;
            if !rel.is_empty() && entries.iter().any(|e| e.file_name.as_str() == "go.mod") {
                continue;
            }
            for entry in entries.iter() {
                let name = entry.file_name.as_str();
                let path = relative_join(&rel, name);
                if entry.file_type == FileType::Directory {
                    level.push(path);
                    continue;
                }
                if name.ends_with(".go") {
                    go_files.push((path.clone(), dir_path.join(&entry.file_name)));
                }
                if !rel.is_empty() && build_file_names.iter().any(|b| b == &entry.file_name) {
                    build_file_paths.push((rel.clone(), dir_path.join(&entry.file_name)));
                }
                files.push((path, None));
            }
        }
    }

    let headers: BTreeMap<String, Arc<str>> = ctx
        .compute_join(go_files, async |ctx: &mut DiceComputations, (rel, path)| {
            let header = ctx
                .compute(&GoFileHeaderKey(Arc::new(path)))
                .await?
                .dupe()?;
            yak_error::Ok((rel, header))
        })
        .await
        .into_iter()
        .collect::<yak_error::Result<_>>()?;
    for (path, header) in &mut files {
        *header = headers.get(path).cloned();
    }
    files.sort();

    let mut build_files = BTreeMap::new();
    for (dir, path) in build_file_paths {
        let contents = DiceFileComputations::read_file_if_exists(ctx, path.as_ref())
            .await?
            .unwrap_or_default();
        // A text search, since the build file is evaluated only after the cell is generated.
        build_files.insert(dir, contents.contains("go_package("));
    }

    Ok(GoModuleInputs {
        go_mod,
        go_sum,
        files,
        build_files,
        go_identity: tool_identity(ctx, "go").await?,
    })
}

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
#[pagable_typetag(dice::DiceKeyDyn)]
struct GoModuleInputsKey(Arc<ProjectRelativePathBuf>);

#[async_trait::async_trait]
impl Key for GoModuleInputsKey {
    type Value = yak_error::Result<Arc<GoModuleInputs>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Ok(Arc::new(read_inputs(ctx, &self.0).await?))
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        // An edit that leaves the inputs unchanged does not run `go list` again.
        EqualityBehavior::Compare(|x, y| match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        })
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        NoValueSerialize::<Self::Value>::new()
    }
}

async fn generate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: &GoCellSetup,
) -> yak_error::Result<GeneratedCellContents> {
    let module = &*setup.module;
    let inputs = ctx
        .compute(&GoModuleInputsKey(setup.module.dupe()))
        .await?
        .dupe()?;

    let cells = ctx.get_cell_resolver().await?;
    let module_dir = module.parent().unwrap_or(ProjectRelativePath::empty());
    let project_root = ctx.global_data().get_io_provider().project_root().dupe();
    let abs_dir = project_root.resolve(module_dir);
    let outputs = futures::future::try_join_all(PLATFORMS.iter().map(|platform| {
        let abs_dir = &abs_dir;
        async move {
            run(
                "go",
                "go",
                &[
                    "list",
                    "-e",
                    "-mod=readonly",
                    "-deps",
                    "-test",
                    "-json",
                    "./...",
                ],
                &[
                    ("GOOS", platform.goos),
                    ("GOARCH", platform.goarch),
                    ("GOWORK", "off"),
                    ("CGO_ENABLED", "1"),
                ],
                abs_dir,
            )
            .await
        }
    }))
    .await?;
    let listings = outputs
        .iter()
        .map(|output| parse_list(output))
        .collect::<yak_error::Result<Vec<_>>>()?;

    let module_cell_path = cells.get_cell_path(module_dir);
    let cell = generate_cell(&ModuleInputs {
        module_dir: module_cell_path.path().as_str(),
        cell: cell_name.as_str(),
        listings: &listings,
        build_files: &inputs.build_files,
    })?;

    let digest_config = ctx
        .global_data()
        .get_digest_config()
        .cas_digest_config()
        .source_files_config();
    Ok(GeneratedCellContents {
        root_files: vec![
            (
                Arc::from(BUILD_FILE),
                GeneratedFile::new(cell.root_build_file, digest_config),
            ),
            (
                Arc::from(MODULE_FILE),
                GeneratedFile::new(cell.module_file, digest_config),
            ),
        ],
        packages: cell
            .modules
            .into_iter()
            .map(|m| GeneratedPackage {
                dir: Arc::from(m.dir),
                source_dir: Arc::from(m.source_dir),
                build_file: GeneratedFile::new(m.build_file, digest_config),
            })
            .collect(),
    })
}

pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: GoCellSetup,
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
    struct GoFileOpsDelegateKey(CellName, GoCellSetup);

    #[async_trait::async_trait]
    impl Key for GoFileOpsDelegateKey {
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
                    ExternalCellOrigin::Go(self.1.dupe()),
                    contents,
                )
                .await?,
            ))
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            // A change to `go list` output that leaves the generated files unchanged does not
            // invalidate the packages of the cell.
            EqualityBehavior::Compare(|x, y| match (x, y) {
                (Ok(x), Ok(y)) => x.same_contents(y),
                _ => false,
            })
        }

        fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
            OkPagableValueSerialize::<Self::Value>::new()
        }
    }

    ctx.compute(&GoFileOpsDelegateKey(cell_name, setup))
        .await?
        .dupe()
}

pub(crate) async fn materialize_all(
    _ctx: &mut DiceComputations<'_>,
    cell: CellName,
    _setup: GoCellSetup,
) -> yak_error::Result<ProjectRelativePathBuf> {
    Err(yak_error::yak_error!(
        yak_error::ErrorTag::Input,
        "The go cell `{cell}` is generated from `go list` and cannot be expanded"
    ))
}
