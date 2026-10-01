/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(once_cell_try)]

use std::sync::Arc;

use async_trait::async_trait;
use dice::DiceComputations;
use yak_common::dice::data::HasIoProvider;
use yak_common::file_ops::delegate::FileOpsDelegate;
use yak_common::file_ops::metadata::RawPathMetadata;
use yak_core::cells::cell_root_path::CellRootPath;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::name::CellName;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;

mod bundled;
mod cargo;
mod generated;
mod git;
mod go;

struct ConcreteExternalCellsImpl;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Tier0)]
enum ExternalCellsError {
    #[error("Tried to expand external cell to `{0}`, but that directory already contains data!")]
    ExpandDataAlreadyPresent(ProjectRelativePathBuf),
}

#[async_trait]
impl yak_common::external_cells::ExternalCellsImpl for ConcreteExternalCellsImpl {
    async fn get_file_ops_delegate(
        &self,
        ctx: &mut DiceComputations<'_>,
        cell_name: CellName,
        origin: ExternalCellOrigin,
    ) -> yak_error::Result<Arc<dyn FileOpsDelegate>> {
        match origin {
            ExternalCellOrigin::Bundled(cell_name) => {
                Ok(bundled::get_file_ops_delegate(ctx, cell_name).await? as _)
            }
            ExternalCellOrigin::Git(setup) => {
                Ok(git::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
            ExternalCellOrigin::Cargo(setup) => {
                Ok(cargo::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
            ExternalCellOrigin::Go(setup) => {
                Ok(go::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
        }
    }

    fn check_bundled_cell_exists(&self, cell_name: CellName) -> yak_error::Result<()> {
        bundled::find_bundled_data(cell_name).map(|_| ())
    }

    async fn expand(
        &self,
        ctx: &mut DiceComputations<'_>,
        cell: CellName,
        origin: ExternalCellOrigin,
        path: &CellRootPath,
    ) -> yak_error::Result<()> {
        let dest_path = path.as_project_relative_path().to_buf();
        let io = ctx.global_data().get_io_provider();

        // Make sure we're not about to overwrite existing data
        match io.read_path_metadata_if_exists(dest_path.clone()).await? {
            None => (),
            Some(RawPathMetadata::Directory) => {
                let data = io.read_dir(dest_path.clone()).await?;
                if !data.is_empty() {
                    return Err(ExternalCellsError::ExpandDataAlreadyPresent(dest_path).into());
                }
            }
            Some(_) => {
                return Err(ExternalCellsError::ExpandDataAlreadyPresent(dest_path).into());
            }
        }

        // Materialize the whole cell, and then copy it into the repository.
        //
        // FIXME(JakobDegen): Ideally we'd be able to ask the materializer to just make a copy
        // without doing the actual materialization. However, that's not currently possible without
        // it resulting in the materializer tracking paths in the repo, so this will have to do for
        // now.
        let materialized_path = match origin {
            ExternalCellOrigin::Bundled(cell) => bundled::materialize_all(ctx, cell).await?,
            ExternalCellOrigin::Git(setup) => git::materialize_all(ctx, cell, setup).await?,
            ExternalCellOrigin::Cargo(setup) => cargo::materialize_all(ctx, cell, setup).await?,
            ExternalCellOrigin::Go(setup) => go::materialize_all(ctx, cell, setup).await?,
        };

        Ok(io.project_root().copy(&materialized_path, &dest_path)?)
    }
}

pub fn init_late_bindings() {
    yak_common::external_cells::EXTERNAL_CELLS_IMPL.init(&ConcreteExternalCellsImpl);
}
