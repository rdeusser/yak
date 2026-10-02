/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::env;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceTransactionUpdater;
use tracing::info;
use yak_common::ignores::ignore_set::IgnoreSet;
use yak_common::legacy_configs::configs::LegacyYakConfig;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_core::cells::CellResolver;
use yak_core::cells::name::CellName;
use yak_core::fs::project::ProjectRoot;
use yak_core::yak_env;
use yak_error::ErrorTag;
use yak_error::YakErrorContext;
use yak_error::yak_error;
use yak_hash::StdYakHashMap;

use crate::dep_files::DepFileCache;
use crate::fs_hash_crawler::FsHashCrawler;
use crate::mergebase::Mergebase;
use crate::notify::NotifyFileWatcher;
use crate::watchman::interface::WatchmanFileWatcher;

#[async_trait]
pub trait FileWatcher: Allocative + Send + Sync + 'static {
    /// Begins the part of the next `sync` that can run while the command loads its
    /// configuration, such as writing a sync marker.
    fn start_sync(&self) {}

    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> yak_error::Result<(DiceTransactionUpdater, Mergebase)>;
}

/// Parse the `dice_clear_on_mergebase_change` config, honoring both the yakconfig
/// and the `YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE` env var override.
pub(crate) fn dice_clear_on_mergebase_change(
    root_config: &LegacyYakConfig,
) -> yak_error::Result<bool> {
    let config_value = root_config
        .parse::<bool>(YakconfigKeyRef {
            section: "yak",
            property: "dice_clear_on_mergebase_change",
        })
        .yak_error_context("Failed to parse dice_clear_on_mergebase_change config")?
        .unwrap_or(true);
    let env_skip = yak_env!(
        "YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE",
        bool,
        applicability = testing
    )
    .yak_error_context("Failed to parse YAK_TEST_SKIP_DICE_CLEAR_ON_MERGEBASE_CHANGE env")?;
    Ok(config_value && !env_skip)
}

/// The context of an error of a Watchman watcher that `yak.file_watcher = auto` selected.
pub(crate) const AUTO_WATCHMAN_HINT: &str = "`yak.file_watcher = auto` selected Watchman because \
     `WATCHMAN_SOCK` is set or `watchman` is on `PATH`. Set `yak.file_watcher = notify` in \
     `.yakconfig` to use the notify watcher";

/// The watcher that `yak.file_watcher` selects.
#[derive(Debug, PartialEq)]
enum Selection {
    Watchman { selected_by_auto: bool },
    Notify,
    FsHashCrawler,
}

impl Selection {
    /// Parses `yak.file_watcher`. `auto`, the default, selects Watchman when it is installed and
    /// the notify watcher otherwise.
    fn parse(
        value: Option<&str>,
        watchman_installed: impl FnOnce() -> bool,
    ) -> yak_error::Result<Selection> {
        match value.unwrap_or("auto") {
            "auto" if watchman_installed() => Ok(Selection::Watchman {
                selected_by_auto: true,
            }),
            "auto" | "notify" => Ok(Selection::Notify),
            "watchman" => Ok(Selection::Watchman {
                selected_by_auto: false,
            }),
            "fs_hash_crawler" => Ok(Selection::FsHashCrawler),
            other => Err(yak_error!(
                ErrorTag::Input,
                "Invalid yak.file_watcher `{other}`. The values are `auto`, `watchman`, \
                 `notify`, and `fs_hash_crawler`"
            )),
        }
    }
}

/// Whether the daemon can reach Watchman: `WATCHMAN_SOCK` names its socket, or `watchman` is on
/// the daemon's `PATH`.
fn watchman_installed() -> bool {
    if env::var_os("WATCHMAN_SOCK").is_some() {
        return true;
    }
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    let name = if cfg!(windows) {
        "watchman.exe"
    } else {
        "watchman"
    };
    env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

impl dyn FileWatcher {
    /// Create a new FileWatcher. Note that this is not async, since it's called during daemon
    /// startup and shouldn't be doing any work that could warrant suspending.
    pub fn new(
        project_root: &ProjectRoot,
        root_config: &LegacyYakConfig,
        cells: CellResolver,
        ignore_specs: StdYakHashMap<CellName, IgnoreSet>,
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

        let value = root_config.get(YakconfigKeyRef {
            section: "yak",
            property: "file_watcher",
        });
        let selection = Selection::parse(value, watchman_installed)?;
        info!("FileWatcher: Selected {selection:?}");

        match selection {
            Selection::Watchman { selected_by_auto } => {
                let watcher = WatchmanFileWatcher::new(
                    project_root.root(),
                    root_config,
                    cells,
                    ignore_specs,
                    dep_file_cache,
                    selected_by_auto,
                )
                .yak_error_context("Creating watchman file watcher");
                let watcher = if selected_by_auto {
                    watcher.yak_error_context(AUTO_WATCHMAN_HINT)?
                } else {
                    watcher?
                };
                Ok(Arc::new(watcher))
            }
            Selection::Notify => Ok(Arc::new(
                NotifyFileWatcher::new(project_root, cells, ignore_specs)
                    .yak_error_context("Creating notify file watcher")?,
            )),
            Selection::FsHashCrawler => Ok(Arc::new(
                FsHashCrawler::new(project_root, cells, ignore_specs)
                    .yak_error_context("Creating fs_crawler file watcher")?,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;

    #[test]
    fn test_selection() {
        let parse = |value, installed| Selection::parse(value, || installed).unwrap();
        assert_eq!(
            parse(None, true),
            Selection::Watchman {
                selected_by_auto: true
            }
        );
        assert_eq!(parse(None, false), Selection::Notify);
        assert_eq!(
            parse(Some("auto"), true),
            Selection::Watchman {
                selected_by_auto: true
            }
        );
        assert_eq!(
            parse(Some("watchman"), false),
            Selection::Watchman {
                selected_by_auto: false
            }
        );
        assert_eq!(parse(Some("notify"), true), Selection::Notify);
        assert_eq!(
            parse(Some("fs_hash_crawler"), true),
            Selection::FsHashCrawler
        );
        assert!(Selection::parse(Some("inotify"), || true).is_err());
    }
}
