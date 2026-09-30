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
use jiff::Timestamp;
use yak_common::invocation_paths::TenantPaths;
use yak_common::legacy_configs::configs::LegacyYakConfig;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_core::rollout_percentage::RolloutPercentage;
use yak_core::soft_error;
use yak_error::YakErrorContext;
use yak_error::YakErrorOptionContext;
use yak_events::daemon_id::DaemonId;
use yak_execute::digest_config::DigestConfig;
use yak_execute::execute::blocking::BlockingExecutor;
use yak_execute_impl::materializers::deferred::DeferredMaterializerConfigs;
use yak_execute_impl::materializers::deferred::clean_stale::DEFAULT_CLEAN_STALE_TTL_DAYS;
use yak_execute_impl::sqlite::dep_file_state_db::DEP_FILE_DB_SCHEMA_VERSION;
use yak_execute_impl::sqlite::dep_file_state_db::DepFileStateSqliteDb;
use yak_execute_impl::sqlite::incremental_state_db::INCREMENTAL_DB_SCHEMA_VERSION;
use yak_execute_impl::sqlite::incremental_state_db::IncrementalDbState;
use yak_execute_impl::sqlite::incremental_state_db::IncrementalStateSqliteDb;
use yak_execute_impl::sqlite::materializer_db::MATERIALIZER_DB_SCHEMA_VERSION;
use yak_execute_impl::sqlite::materializer_db::MaterializerState;
use yak_execute_impl::sqlite::materializer_db::MaterializerStateSqliteDb;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_fs::paths::file_name::FileName;
use yak_hash::IntentionallyStdHashMap;

use crate::daemon::server::RepoStateInitPreferences;

#[derive(Allocative)]
pub struct DiskStateOptions {
    pub sqlite_materializer_state: bool,
}

impl DiskStateOptions {
    pub fn new(root_config: &LegacyYakConfig) -> yak_error::Result<Self> {
        let sqlite_materializer_state = root_config
            .parse::<RolloutPercentage>(YakconfigKeyRef {
                section: "yak",
                property: "sqlite_materializer_state",
            })?
            .unwrap_or_else(RolloutPercentage::always)
            .roll();
        Ok(Self {
            sqlite_materializer_state,
        })
    }
}

fn sqlite_db_setup_metadata_and_versions(
    root_config: &LegacyYakConfig,
    schema_version: String,
    version_config: &str,
    deferred_materializer_config: Option<&DeferredMaterializerConfigs>,
    daemon_id: &DaemonId,
) -> yak_error::Result<(
    IntentionallyStdHashMap<String, String>,
    IntentionallyStdHashMap<String, String>,
)> {
    let metadata = yak_events::metadata::collect(daemon_id);

    let mut versions =
        IntentionallyStdHashMap::from([("schema_version".to_owned(), schema_version)]);

    if let Some(config) = deferred_materializer_config {
        versions.insert(
            "defer_write_actions".to_owned(),
            config.defer_write_actions.to_string(),
        );
    }

    if let Some(yakconfig_version) = root_config.parse(YakconfigKeyRef {
        section: "yak",
        property: version_config,
    })? {
        versions.insert("yakconfig_version".to_owned(), yakconfig_version);
    }
    if let Some(hostname) = metadata.get("hostname") {
        versions.insert("hostname".to_owned(), hostname.to_owned());
    }

    Ok((metadata, versions))
}

pub(crate) async fn maybe_initialize_materializer_sqlite_db(
    options: &DiskStateOptions,
    paths: TenantPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyYakConfig,
    deferred_materializer_configs: &DeferredMaterializerConfigs,
    digest_config: DigestConfig,
    init_ctx: &RepoStateInitPreferences,
    daemon_id: &DaemonId,
) -> yak_error::Result<(Option<MaterializerStateSqliteDb>, Option<MaterializerState>)> {
    if !options.sqlite_materializer_state {
        // When sqlite materializer state is disabled, we should always delete the materializer state db.
        // Otherwise, artifacts in yak-out will diverge from the state stored in db.
        io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.materializer_state_path())
                    .categorize_internal()
                    .map_err(yak_error::Error::from)
            })
            .await?;
        return Ok((None, None));
    }

    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        root_config,
        MATERIALIZER_DB_SCHEMA_VERSION.to_string(),
        "sqlite_materializer_state_version",
        Some(deferred_materializer_configs),
        daemon_id,
    )?;

    // Most things in the rest of `metadata` should go in the metadata sqlite table.
    // TODO(scottcao): Narrow down what metadata we need and insert them into the
    // metadata table before a feature rollout.
    let (db, load_result) = MaterializerStateSqliteDb::initialize(
        paths.materializer_state_path(),
        versions,
        metadata,
        io_executor,
        digest_config,
        init_ctx.reject_materializer_state.as_ref(),
    )
    .await?;

    // We know path not found or version mismatch is normal, but some sqlite failures
    // are worth logging here. TODO(scottcao): Refine our error types and figure out what
    // errors to log
    let materializer_state = load_result.ok();
    Ok((Some(db), materializer_state))
}

pub(crate) async fn maybe_initialize_incremental_sqlite_db(
    paths: TenantPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyYakConfig,
    daemon_id: &DaemonId,
) -> yak_error::Result<IncrementalDbState> {
    // Rolling it out by default, but giving an option to disable in case something goes horribly wrong
    if !root_config
        .parse(YakconfigKeyRef {
            section: "yak",
            property: "sqlite_incremental_state",
        })?
        .unwrap_or(true)
    {
        // When sqlite incremental state is disabled, we should always delete the db to
        // prevent futures invocations from potentially using stale entries
        io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.incremental_state_path())
                    .categorize_internal()
                    .map_err(yak_error::Error::from)
            })
            .await?;
        return Ok(IncrementalDbState::db_disabled());
    }

    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        root_config,
        INCREMENTAL_DB_SCHEMA_VERSION.to_string(),
        "sqlite_incremental_state_version",
        None,
        daemon_id,
    )?;

    let incremental_db_state = IncrementalStateSqliteDb::initialize(
        paths.incremental_state_path(),
        versions,
        metadata,
        io_executor,
        // TODO(minglunli): I'm not convinced we need reject_identity for incremental state. iiuc, this is only used by restarter
        // but incremental state isn't as widely used as materializer so we prob shouldn't restart daemon even if that's out of sync?
        None,
    )
    .await?;
    Ok(incremental_db_state)
}

pub(crate) async fn maybe_initialize_dep_file_sqlite_db(
    options: &DiskStateOptions,
    paths: TenantPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyYakConfig,
    daemon_id: &DaemonId,
) -> yak_error::Result<Option<DepFileStateSqliteDb>> {
    // On unless `yak.sqlite_dep_file_state = false`, but only meaningful with the materializer
    // state db. A cross-restart hit re-validates outputs via `Materializer::declare_match`, which
    // after a restart only reports a match if the materializer reloaded its tracked state from
    // sqlite. Without `sqlite_materializer_state` that tree is empty post-restart, so no reloaded
    // entry could ever hit and persisting them would be pure overhead.
    let configured: Option<bool> = root_config.parse(YakconfigKeyRef {
        section: "yak",
        property: "sqlite_dep_file_state",
    })?;
    let requested = configured.unwrap_or(true);
    if configured == Some(true) && !options.sqlite_materializer_state {
        tracing::warn!(
            "Ignoring `yak.sqlite_dep_file_state`: it needs `yak.sqlite_materializer_state`, \
             which is disabled. The persisted dep-file cache re-validates outputs against the \
             materializer state db after a restart. Startup continues with the cache disabled."
        );
    }
    let enabled = requested && options.sqlite_materializer_state;
    if !enabled {
        // When disabled, delete the db so a future enabled invocation can't use stale entries. A
        // failure here must not take the daemon down for a feature that is off: a db that survives
        // still cannot serve a stale entry, since every reloaded entry is re-validated against the
        // action's digests and the materializer before use.
        let removed = io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.dep_file_state_path())
                    .categorize_internal()
                    .map_err(yak_error::Error::from)
            })
            .await;
        if let Err(e) = removed {
            let _unused = soft_error!(
                "dep_file_state_db_remove",
                yak_error::yak_error!(
                    yak_error::ErrorTag::Tier0,
                    "Failed to delete the disabled dep-file state db. {}",
                    e
                ),
                quiet: true
            );
        }
        return Ok(None);
    }

    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        root_config,
        DEP_FILE_DB_SCHEMA_VERSION.to_string(),
        "sqlite_dep_file_state_version",
        None,
        daemon_id,
    )?;

    // Bound the db across sessions. TTL (0 disables age-based pruning) mirrors the materializer's
    // default `clean_stale_artifact_ttl_hours`; `max_entries` is an optional hard cap.
    let ttl_days: u64 = root_config
        .parse(YakconfigKeyRef {
            section: "yak",
            property: "sqlite_dep_file_state_ttl_days",
        })?
        .unwrap_or(DEFAULT_CLEAN_STALE_TTL_DAYS);
    let prune_cutoff = if ttl_days == 0 {
        None
    } else {
        // `ttl_days` comes from a yakconfig, so it can be absurd. Saturate the multiply and clamp
        // to `i64::MAX` before the cast, since `u64::MAX as i64` would otherwise wrap negative and
        // prune everything. Clamping just means "prune nothing", which is what an absurdly long TTL
        // asks for anyway.
        let ttl_seconds = ttl_days.saturating_mul(24 * 60 * 60).min(i64::MAX as u64) as i64;
        Some(Timestamp::now().as_second().saturating_sub(ttl_seconds))
    };
    let max_entries: Option<usize> = root_config.parse(YakconfigKeyRef {
        section: "yak",
        property: "sqlite_dep_file_state_max_entries",
    })?;

    // A cache that fails safe to a miss on every lookup should not keep the daemon from
    // starting because its db will not open, so a failure here disables persistence for the session
    // instead of propagating. This mirrors the install site, which treats a store that cannot be
    // built the same way.
    let db = match DepFileStateSqliteDb::initialize(
        paths.dep_file_state_path(),
        versions,
        metadata,
        io_executor,
        // Like incremental state, the dep-file cache fails safe to a miss on every lookup, so it
        // does not need identity rejection for the restarter.
        None,
        prune_cutoff,
        max_entries,
    )
    .await
    {
        Ok(db) => db,
        Err(e) => {
            let _unused = soft_error!(
                "dep_file_state_db_init",
                yak_error::yak_error!(
                    yak_error::ErrorTag::Tier0,
                    "Failed to open the dep-file state db; continuing without persistence. {}",
                    e
                ),
                quiet: true
            );
            return Ok(None);
        }
    };
    Ok(Some(db))
}

// Once we start storing disk state in the cache directory, we need to make sure
// yak always deletes the cache directory if the cache is disabled.
// Otherwise, yak-out state can diverge from the state of on-disk cache when
// cache is disabled, causing yak to use stale cache when reading from the
// cache is re-enabled. One way this can happen is that someone can build on
// an older revision with a yak that doesn't understand the cache directory
// in between 2 builds on newer revisions with yak that reads from the cache
// (for ex., as a part of a bisect), then the state can become stale.
// There are 2 (not foolproof) mitigations planned:
// 1) Read from the logs what the last yak invocation was and check that the
// last yak supported on-disk state. If not, delete the disk state.
// 2) Start always deleting the cache directory now until we add support for disk
// state in yak.
// The following implements mitigation #2 by always deleting disk state.

/// Recursively deletes all elements under `cache_dir_path`, except for known dirs
/// listed in `known_dir_names`.
pub(crate) fn delete_unknown_disk_state(
    cache_dir_path: &AbsNormPath,
    known_dir_names: &[&FileName],
) -> yak_error::Result<()> {
    let res: yak_error::Result<()> = try {
        if cache_dir_path.exists() {
            for entry in fs_util::read_dir(cache_dir_path).categorize_internal()? {
                let entry = entry.map_err(yak_error::Error::from)?;
                let filename = entry.file_name();
                let filename = filename
                    .to_str()
                    .internal_error("Filename is not UTF-8")
                    .and_then(FileName::new)?;

                // known_dir_names is always small, so this contains isn't expensive
                if !known_dir_names.contains(&filename) || !entry.path().is_dir() {
                    fs_util::remove_all(cache_dir_path.join(filename)).categorize_internal()?;
                }
            }
        }
    };

    res.with_yak_error_context(|| {
        format!(
            "deleting unrecognized caches in {} to prevent them from going stale",
            cache_dir_path
        )
    })
}

#[cfg(test)]
mod tests {
    use yak_core::fs::project::ProjectRootTemp;
    use yak_core::fs::project_rel_path::ProjectRelativePath;
    use yak_fs::fs_util::uncategorized as fs_util;
    use yak_fs::paths::forward_rel_path::ForwardRelativePath;

    use super::*;

    #[test]
    fn test_delete_all_from_cache_dir() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("yak-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(&cache_dir_path, &[]).unwrap();

        assert!(!materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }

    #[test]
    fn test_delete_from_cache_dir_with_known_dirs() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("yak-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(
            &cache_dir_path,
            &[FileName::unchecked_new("materializer_state")],
        )
        .unwrap();

        assert!(materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }
}
