/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceTransactionUpdater;
use yak_common::ignores::ignore_set::IgnoreSet;
use yak_common::legacy_configs::configs::LegacyBuckConfig;
use yak_common::legacy_configs::key::BuckconfigKeyRef;
use yak_core::cells::CellResolver;
use yak_core::cells::name::CellName;
use yak_core::fs::project::ProjectRoot;
use yak_core::yak_env;
use yak_error::BuckErrorContext;
use yak_error::ErrorTag;
use yak_error::yak_error;
use yak_hash::StdBuckHashMap;

use crate::dep_files::DepFileCache;
use crate::fs_hash_crawler::FsHashCrawler;
use crate::mergebase::Mergebase;
use crate::notify::NotifyFileWatcher;
use crate::watchman::interface::WatchmanFileWatcher;

#[async_trait]
pub trait FileWatcher: Allocative + Send + Sync + 'static {
    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> yak_error::Result<(DiceTransactionUpdater, Mergebase)>;
}

/// Parse the `dice_clear_on_mergebase_change` config, honoring both the yakconfig
/// and the `YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE` env var override.
pub(crate) fn dice_clear_on_mergebase_change(
    root_config: &LegacyBuckConfig,
) -> yak_error::Result<bool> {
    let config_value = root_config
        .parse::<bool>(BuckconfigKeyRef {
            section: "yak",
            property: "dice_clear_on_mergebase_change",
        })
        .buck_error_context("Failed to parse dice_clear_on_mergebase_change config")?
        .unwrap_or(true);
    let env_skip = yak_env!(
        "YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE",
        bool,
        applicability = testing
    )
    .buck_error_context("Failed to parse YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE env")?;
    Ok(config_value && !env_skip)
}

impl dyn FileWatcher {
    /// Create a new FileWatcher. Note that this is not async, since it's called during daemon
    /// startup and shouldn't be doing any work that could warrant suspending.
    pub fn new(
        project_root: &ProjectRoot,
        root_config: &LegacyBuckConfig,
        cells: CellResolver,
        ignore_specs: StdBuckHashMap<CellName, IgnoreSet>,
        dep_file_cache: Arc<dyn DepFileCache>,
    ) -> yak_error::Result<Arc<dyn FileWatcher>> {
        if !project_root.root().as_path().exists() {
            return Err(yak_error!(
                ErrorTag::MissingProjectRoot,
                "Project root `{}` does not exist. \
                 The directory may have been removed.",
                project_root.root()
            ));
        }

        let default = "notify";

        let watcher_conf = root_config
            .get(BuckconfigKeyRef {
                section: "yak",
                property: "file_watcher",
            })
            .unwrap_or(default);

        match watcher_conf {
            "watchman" => Ok(Arc::new(
                WatchmanFileWatcher::new(
                    project_root.root(),
                    root_config,
                    cells,
                    ignore_specs,
                    dep_file_cache,
                )
                .buck_error_context("Creating watchman file watcher")?,
            )),
            "notify" => Ok(Arc::new(
                NotifyFileWatcher::new(project_root, cells, ignore_specs)
                    .buck_error_context("Creating notify file watcher")?,
            )),
            "fs_hash_crawler" => Ok(Arc::new(
                FsHashCrawler::new(project_root, cells, ignore_specs)
                    .buck_error_context("Creating fs_crawler file watcher")?,
            )),
            other => Err(yak_error!(
                yak_error::ErrorTag::Tier0,
                "Invalid yak.file_watcher: {}",
                other
            )),
        }
    }
}
