/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::future::Future;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

use allocative::Allocative;
use dupe::Dupe;
use gazebo::prelude::*;
use gazebo::variants::VariantName;
use host_sharing::NamedSemaphores;
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tracing::Instrument;
use yak_build_api::spawner::YakSpawner;
use yak_cli_proto::ClientContext;
use yak_cli_proto::unstable_dice_dump_request::DiceDumpFormat;
use yak_common::cas_digest::DigestAlgorithm;
use yak_common::cas_digest::DigestAlgorithmFamily;
use yak_common::ignores::ignore_set::IgnoreSet;
use yak_common::init::DaemonStartupConfig;
use yak_common::init::SystemWarningConfig;
use yak_common::init::Timeout;
use yak_common::invocation_paths::InvocationPaths;
use yak_common::invocation_paths::TenantPaths;
use yak_common::io::IoProvider;
use yak_common::legacy_configs::cells::YakConfigBasedCells;
use yak_common::legacy_configs::configs::LegacyYakConfig;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_common::legacy_configs::parse_yakconfig_metadata;
use yak_common::sqlite::sqlite_db::SqliteDb;
use yak_common::sqlite::sqlite_db::SqliteIdentity;
use yak_common::tenant::TenantKey;
use yak_common::tenant::TenantSpec;
use yak_core::cells::name::CellName;
use yak_core::fs::project::ProjectRoot;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::rollout_percentage::RolloutPercentage;
use yak_core::soft_error;
use yak_core::tag_result;
use yak_core::yak_env;
use yak_error::ErrorTag;
use yak_error::YakErrorContext;
use yak_error::yak_error;
use yak_events::daemon_id::DaemonId;
use yak_events::dispatch::EventDispatcher;
use yak_events::source::ChannelEventSource;
use yak_execute::dep_file_state::DepFileStore;
use yak_execute::digest_config::DigestConfig;
use yak_execute::execute::blocking::BlockingExecutor;
use yak_execute::execute::blocking::BlockingExecutorFactory;
use yak_execute::materialize::materializer::FinalArtifactMaterialization;
use yak_execute::materialize::materializer::Materializer;
use yak_execute::re::manager::ReConnectionManager;
use yak_execute_impl::executors::local::ForkserverAccess;
use yak_execute_impl::materializers::deferred::AccessTimesUpdates;
use yak_execute_impl::materializers::deferred::DeferredMaterializer;
use yak_execute_impl::materializers::deferred::DeferredMaterializerConfigs;
use yak_execute_impl::materializers::deferred::TtlRefreshConfiguration;
use yak_execute_impl::materializers::deferred::clean_stale::CleanStaleConfig;
use yak_execute_impl::re::paranoid_download::ParanoidDownloader;
use yak_execute_impl::sqlite::dep_file_state_db::PersistedDepFileStore;
use yak_execute_impl::sqlite::incremental_state_db::IncrementalDbState;
use yak_execute_impl::sqlite::materializer_db::MaterializerState;
use yak_execute_impl::sqlite::materializer_db::MaterializerStateSqliteDb;
use yak_file_watcher::dep_files::DepFileCache;
use yak_file_watcher::dep_files::create_dep_file_cache;
use yak_file_watcher::file_watcher::FileWatcher;
use yak_fs::cwd::WorkingDirectory;
use yak_fs::paths::abs_norm_path::AbsNormPathBuf;
use yak_fs::paths::file_name::FileNameBuf;
use yak_hash::StdYakHashMap;
use yak_http::HttpClient;
use yak_http::HttpClientBuilder;
use yak_re_configuration::RemoteExecutionStaticMetadata;
use yak_re_configuration::RemoteExecutionStaticMetadataImpl;
use yak_resource_control::memory_tracker;
use yak_resource_control::memory_tracker::MemoryTrackerHandle;
use yak_resource_control::yak_cgroup_tree::YakCgroupTree;
use yak_server_ctx::concurrency::ConcurrencyHandler;
use yak_server_ctx::ctx::LockedPreviousCommandData;
use yak_wrapper_common::invocation_id::TraceId;

use crate::active_commands::ActiveCommandDropGuard;
use crate::ctx::BaseServerCommandContext;
use crate::daemon::check_working_dir;
use crate::daemon::disk_state::DiskStateOptions;
use crate::daemon::disk_state::delete_unknown_disk_state;
use crate::daemon::disk_state::maybe_initialize_dep_file_sqlite_db;
use crate::daemon::disk_state::maybe_initialize_incremental_sqlite_db;
use crate::daemon::disk_state::maybe_initialize_materializer_sqlite_db;
use crate::daemon::forkserver::maybe_launch_forkserver;
use crate::daemon::io_provider::create_io_provider;
use crate::daemon::server::TenantStateInitPreferences;
use crate::daemon::server::YakdServerInitPreferences;
use crate::paging::PageOutThresholds;
use crate::snapshot::DepFileDbSizeSampler;

/// For a yakd process there is a single DaemonState created at startup and never destroyed.
#[derive(Allocative)]
pub struct DaemonState {
    /// This holds the main data shared across different commands.
    pub(crate) data: Arc<DaemonStateData>,

    /// Our working directory.
    working_directory: WorkingDirectory,
}

#[derive(Allocative)]
pub(crate) struct PersistedDepFileCache {
    #[allocative(skip)]
    pub(crate) store: Arc<dyn DepFileStore>,

    #[allocative(skip)]
    pub(crate) db_size: Arc<DepFileDbSizeSampler>,
}

/// State scoped to one logical tenant.
///
/// A tenant owns the DICE command state for one `(project root, isolation)` pair and refers to the
/// repository services used by that state. Keeping the two levels explicit allows repository state
/// to be shared independently of tenant-specific DICE state.
#[derive(Allocative)]
pub struct TenantState {
    pub repo: Arc<RepoState>,

    /// The DICE graph and the command concurrency state for this tenant.
    pub(crate) dice_manager: Arc<ConcurrencyHandler>,
}

/// Services and caches associated with a repository-backed tenant.
///
/// These are nested under [`TenantState`] even while each tenant constructs a distinct instance.
/// That ownership boundary leaves room for selected repository services to be shared later.
#[derive(Allocative)]
pub struct RepoState {
    /// Stable paths used by tenant services. Invocation cwd is not stored here.
    pub paths: TenantPaths,

    /// Synced every time we run a command.
    pub(crate) file_watcher: Arc<dyn FileWatcher>,

    /// Settled every time we run a command.
    pub io: Arc<dyn IoProvider>,

    /// Most materializations go through the materializer, providing a single point
    /// where the most expensive network and fs IO operations are performed. It
    /// needs access to the `ReConnectionManager` to download from RE. It must
    /// live for the entire lifetime of the daemon, in order to allow deferred
    /// materializations to work properly between distinct build commands.
    pub(crate) materializer: Arc<dyn Materializer>,

    /// Whether to consult the offline-cache yak-out dir for network action
    /// outputs prior to running them. If no cached output exists, the action
    /// (download_file, cas_artifact) will execute normally.
    ///
    /// This supports fully-offline builds, where network actions like
    /// download_file have an execution component that is inherently non-local (
    /// e.g. making a HEAD request against the remote artifact to determine if
    /// it needs to be downloaded again).
    pub use_network_action_output_cache: bool,

    /// Whether a command selecting this repo should ask the client to restart the daemon after an
    /// error.
    pub restart_daemon_on_error: bool,

    /// What yak state to store on disk, ex. materializer state on sqlite
    pub disk_state_options: DiskStateOptions,

    #[allocative(skip)]
    pub create_unhashed_outputs_lock: Arc<Mutex<()>>,

    /// A unique identifier for the materializer state.
    pub materializer_state_identity: Option<SqliteIdentity>,

    /// Tracks data about previous command (e.g. configs)
    pub previous_command_data: Arc<LockedPreviousCommandData>,

    /// State of the Incremental Action DB for content-based hash paths
    #[allocative(skip)]
    pub incremental_db_state: Arc<IncrementalDbState>,

    /// Persisted local dep-file cache and its size sampler for this repo, if enabled.
    pub(crate) persisted_dep_file_cache: Option<PersistedDepFileCache>,

    /// Live local dep-file cache for this repo.
    pub dep_file_cache: Arc<dyn DepFileCache>,

    /// If enabled, paranoid RE downloads.
    pub paranoid: Option<ParanoidDownloader>,

    /// The RE connection for this repo, managed such that all concurrently active build commands
    /// use the same connection. Once there are no active build commands, the connection is
    /// terminated.
    pub re_client_manager: Arc<ReConnectionManager>,

    /// Executor for blocking I/O against this repo's project root.
    pub blocking_executor: Arc<dyn BlockingExecutor>,

    pub yakconfig_metadata: StdYakHashMap<String, String>,

    /// Tags to be logged per command.
    pub tags: Vec<String>,

    /// Config used to display system warnings
    pub system_warning_config: SystemWarningConfig,

    /// Whether a finishing command schedules a background sweep of the local-action
    /// scratch dirs (`yak-out/<iso>/tmp*`) once the daemon is idle
    /// (`yak.clean_scratch_on_idle`). The sweep runs through this repo's `materializer`.
    pub(crate) clean_scratch_on_idle: bool,

    /// Resource-pressure thresholds for automatic idle page-out, selected for this tenant's
    /// isolation. `None` disables automatic idle page-out for this tenant.
    pub(crate) page_out_on_idle: Option<PageOutThresholds>,
}

struct TenantStateInit<'a> {
    paths: TenantPaths,
    init_ctx: &'a TenantStateInitPreferences,
    legacy_cells: &'a YakConfigBasedCells,
    root_config: &'a LegacyYakConfig,
    final_artifact_materialization: FinalArtifactMaterialization,
    runtime: &'a Handle,
    shared: DaemonSharedServices<'a>,
}

struct DaemonSharedServices<'a> {
    blocking_executor_factory: &'a BlockingExecutorFactory,
    memory_tracker: Option<&'a MemoryTrackerHandle>,
    daemon_id: &'a DaemonId,
}

#[derive(Allocative)]
struct TenantStateFactory {
    init_ctx: TenantStateInitPreferences,
    #[allocative(skip)]
    final_artifact_materialization: FinalArtifactMaterialization,
    #[allocative(skip)]
    runtime: Handle,
}

impl TenantStateFactory {
    async fn create(
        &self,
        paths: TenantPaths,
        shared: DaemonSharedServices<'_>,
    ) -> yak_error::Result<Arc<TenantState>> {
        let yak_out_path = paths.yak_out_path();
        tokio::fs::create_dir_all(&yak_out_path)
            .await
            .tag(ErrorTag::InvalidYakOut)
            .yak_error_context("Error creating yak_out_path")?;

        let fs = paths.project_root().clone();
        let legacy_cells = YakConfigBasedCells::parse_with_config_args(&fs, &[]).await?;
        let cells = &legacy_cells.cell_resolver;
        let root_config = &legacy_cells
            .parse_single_cell(cells.root_cell(), &fs)
            .await?;

        self.create_with_loaded_config(paths, &legacy_cells, root_config, shared)
            .await
    }

    async fn create_with_loaded_config(
        &self,
        paths: TenantPaths,
        legacy_cells: &YakConfigBasedCells,
        root_config: &LegacyYakConfig,
        shared: DaemonSharedServices<'_>,
    ) -> yak_error::Result<Arc<TenantState>> {
        TenantState::create(TenantStateInit {
            paths,
            init_ctx: &self.init_ctx,
            legacy_cells,
            root_config,
            final_artifact_materialization: self.final_artifact_materialization,
            runtime: &self.runtime,
            shared,
        })
        .await
    }
}

impl TenantState {
    async fn create(init: TenantStateInit<'_>) -> yak_error::Result<Arc<Self>> {
        let TenantStateInit {
            paths,
            init_ctx,
            legacy_cells,
            root_config,
            final_artifact_materialization,
            runtime,
            shared,
        } = init;
        let fs = paths.project_root().clone();
        let cells = &legacy_cells.cell_resolver;

        let default_digest_algorithm =
            yak_env!("YAK_DEFAULT_DIGEST_ALGORITHM", type=DigestAlgorithmFamily)?;

        let default_digest_algorithm =
            default_digest_algorithm.unwrap_or(DigestAlgorithmFamily::Sha256);

        let digest_algorithms = init_ctx
            .daemon_startup_config
            .digest_algorithms
            .as_ref()
            .map(|algos| {
                algos
                    .split(',')
                    .map(DigestAlgorithmFamily::from_str)
                    .collect::<Result<_, _>>()
            })
            .transpose()
            .yak_error_context("Invalid digest_algorithms")?
            .unwrap_or_else(|| vec![default_digest_algorithm])
            .into_try_map(convert_algorithm_kind)?;

        let preferred_source_algorithm = init_ctx
            .daemon_startup_config
            .source_digest_algorithm
            .as_deref()
            .map(|a| convert_algorithm_kind(a.parse()?))
            .transpose()
            .yak_error_context("Invalid source_digest_algorithm")?;

        let digest_config = DigestConfig::leak_new(digest_algorithms, preferred_source_algorithm)
            .yak_error_context("Error initializing DigestConfig")?;

        // TODO(rafaelc): merge configs from all cells once they are consistent
        let static_metadata = Arc::new(RemoteExecutionStaticMetadata::from_legacy_config(
            root_config,
        )?);

        let mut ignore_specs: StdYakHashMap<CellName, IgnoreSet> = StdYakHashMap::default();
        for (cell, _) in cells.cells() {
            let config = legacy_cells.parse_single_cell(cell, &fs).await?;
            ignore_specs.insert(
                cell,
                IgnoreSet::from_ignore_spec(
                    config
                        .get(YakconfigKeyRef {
                            section: "project",
                            property: "ignore",
                        })
                        .unwrap_or(""),
                    cells.is_root_cell(cell),
                )?,
            );
        }

        let disk_state_options = DiskStateOptions::new(root_config)?;
        let blocking_executor = shared.blocking_executor_factory.for_project(fs.dupe());

        let cache_dir_path = paths.cache_dir_path();
        let valid_cache_dirs = paths.valid_cache_dirs();

        let deferred_materializer_configs = {
            let defer_write_actions = root_config
                .parse::<RolloutPercentage>(YakconfigKeyRef {
                    section: "yak",
                    property: "defer_write_actions",
                })?
                .unwrap_or_else(RolloutPercentage::never)
                .roll();

            // RE will refresh any TTL < 1 hour, so we check twice an hour and refresh any TTL
            // < 1 hour.
            let ttl_refresh_frequency = root_config
                .parse(YakconfigKeyRef {
                    section: "yak",
                    property: "ttl_refresh_frequency_seconds",
                })?
                .unwrap_or(1800);

            let ttl_refresh_min_ttl = root_config
                .parse(YakconfigKeyRef {
                    section: "yak",
                    property: "ttl_refresh_min_ttl_seconds",
                })?
                .unwrap_or(3600);

            let ttl_refresh_enabled = root_config
                .parse::<RolloutPercentage>(YakconfigKeyRef {
                    section: "yak",
                    property: "ttl_refresh_enabled",
                })?
                .unwrap_or_else(RolloutPercentage::never)
                .roll();

            let update_access_times =
                AccessTimesUpdates::try_new_from_config_value(root_config.get(YakconfigKeyRef {
                    section: "yak",
                    property: "update_access_times",
                }))?;

            let verbose_materializer_log = root_config
                .parse(YakconfigKeyRef {
                    section: "yak",
                    property: "verbose_materializer_event_log",
                })?
                .unwrap_or(false);

            let mut clean_stale_config = CleanStaleConfig::from_yak_config(root_config)?;
            clean_stale_config.suppress_unmaterialize_without_ttl_refresh(ttl_refresh_enabled);

            DeferredMaterializerConfigs {
                materialize_final_artifacts: matches!(
                    final_artifact_materialization,
                    FinalArtifactMaterialization::Enabled
                ),
                defer_write_actions,
                ttl_refresh: TtlRefreshConfiguration {
                    frequency: Duration::from_secs(ttl_refresh_frequency),
                    min_ttl: jiff::SignedDuration::from_secs(ttl_refresh_min_ttl),
                    enabled: ttl_refresh_enabled,
                },
                update_access_times,
                verbose_materializer_log,
                clean_stale_config,
            }
        };

        tracing::info!("Creating materializer...");
        let (io, _, (materializer_db, materializer_state), incremental_db_state, dep_file_db) =
            futures::future::try_join5(
                create_io_provider(
                    fs.dupe(),
                    digest_config.cas_digest_config(),
                    init_ctx.enable_trace_io,
                ),
                (blocking_executor.dupe() as Arc<dyn BlockingExecutor>).execute_io_inline(|| {
                    // Using `execute_io_inline` is just out of convenience.
                    // It doesn't really matter what's used here since there's no IO-heavy
                    // operations on daemon startup.
                    delete_unknown_disk_state(&cache_dir_path, &valid_cache_dirs)
                }),
                maybe_initialize_materializer_sqlite_db(
                    &disk_state_options,
                    paths.clone(),
                    blocking_executor.dupe() as Arc<dyn BlockingExecutor>,
                    root_config,
                    &deferred_materializer_configs,
                    digest_config,
                    init_ctx,
                    shared.daemon_id,
                ),
                maybe_initialize_incremental_sqlite_db(
                    paths.clone(),
                    blocking_executor.dupe() as Arc<dyn BlockingExecutor>,
                    root_config,
                    shared.daemon_id,
                ),
                maybe_initialize_dep_file_sqlite_db(
                    &disk_state_options,
                    paths.clone(),
                    blocking_executor.dupe() as Arc<dyn BlockingExecutor>,
                    root_config,
                    shared.daemon_id,
                ),
            )
            .await?;

        // The cache is best-effort, so a store that cannot be built leaves the
        // repo running without persistence rather than failing startup.
        let persisted_dep_file_cache = dep_file_db.and_then(|dep_file_db| {
            match PersistedDepFileStore::try_new(dep_file_db, digest_config) {
                Ok(store) => {
                    let store = Arc::new(store) as Arc<dyn DepFileStore>;
                    Some(PersistedDepFileCache {
                        db_size: DepFileDbSizeSampler::start(store.dupe(), runtime),
                        store,
                    })
                }
                Err(e) => {
                    let _unused = soft_error!(
                        "dep_file_store_init",
                        yak_error::yak_error!(
                            yak_error::ErrorTag::Tier0,
                            "Failed to start the persisted dep-file cache; continuing without \
                             it. {}",
                            e
                        ),
                        quiet: true
                    );
                    None
                }
            }
        });
        let incremental_db_state = Arc::new(incremental_db_state);
        let materializer_state_identity = materializer_db.as_ref().map(|d| d.identity().clone());

        let re_client_manager =
            Arc::new(ReConnectionManager::new(false, 10, static_metadata.dupe()));
        let materializer = RepoState::create_materializer(
            io.project_root().dupe(),
            digest_config,
            paths.yak_out_dir(),
            re_client_manager.dupe(),
            blocking_executor.dupe(),
            deferred_materializer_configs,
            materializer_db,
            materializer_state,
            // Events the materializer emits outside any command (such as background cleanup) have
            // no client to reach.
            EventDispatcher::null(),
        )?;

        tracing::info!("Constructing DICE...");
        let dice = init_ctx
            .construct_dice(
                io.dupe(),
                digest_config,
                root_config,
                paths.dice_state_path().as_ref(),
            )
            .await?;

        let dep_file_cache = create_dep_file_cache();

        tracing::info!("Creating file watcher...");
        let file_watcher = <dyn FileWatcher>::new(
            paths.project_root(),
            root_config,
            cells.dupe(),
            ignore_specs,
            dep_file_cache.dupe(),
        )
        .with_yak_error_context(|| {
            format!(
                "Error creating a FileWatcher for project root `{}`",
                paths.project_root()
            )
        })?;

        // TODO(bobyf): Eagerly sync the file watcher here once the DICE commit panic is fixed.

        let use_network_action_output_cache = root_config
            .parse(YakconfigKeyRef {
                section: "yak",
                property: "use_network_action_output_cache",
            })?
            .unwrap_or(false);

        let paranoid = if init_ctx.daemon_startup_config.paranoid {
            Some(ParanoidDownloader::new(
                fs.clone(),
                blocking_executor.dupe(),
                re_client_manager.dupe(),
                paths.paranoid_cache_dir(),
            ))
        } else {
            None
        };

        let remote_dep_files_enabled = root_config
            .parse(YakconfigKeyRef {
                section: "build",
                property: "remote_dep_file_cache_enabled",
            })?
            .unwrap_or(false);

        let action_freezing_enabled = init_ctx
            .daemon_startup_config
            .resource_control
            .enable_suspension;

        let page_out_on_idle = init_ctx
            .daemon_startup_config
            .idle_page_out_config_for_isolation_dir(paths.isolation())
            .map(|hydration| PageOutThresholds {
                min_free_disk_gb: hydration.page_out_min_free_disk_gb,
            });

        let tags = vec![
            format!("dice-detect-cycles:{}", dice.detect_cycles().variant_name()),
            // TODO(scottcao): Delete this tag since now hash all commands is always enabled.
            "hash-all-commands:true".to_owned(),
            format!(
                "sqlite-materializer-state:{}",
                disk_state_options.sqlite_materializer_state
            ),
            format!("paranoid:{}", paranoid.is_some()),
            format!("remote-dep-files:{}", remote_dep_files_enabled),
            "disable-eager-write-dispatch-v2:true".to_owned(),
            format!("memory_tracker-enabled:{}", shared.memory_tracker.is_some()),
            format!("action-freezing-enabled:{}", action_freezing_enabled),
            format!("has-cgroup:{}", shared.memory_tracker.is_some()),
        ];

        let dice_manager = ConcurrencyHandler::new(dice);
        let repo = Arc::new(RepoState {
            paths,
            file_watcher,
            io,
            materializer,
            use_network_action_output_cache,
            restart_daemon_on_error: root_config
                .parse::<RolloutPercentage>(YakconfigKeyRef {
                    section: "yak",
                    property: "restarter",
                })?
                .unwrap_or_else(RolloutPercentage::never)
                .roll(),
            disk_state_options,
            create_unhashed_outputs_lock: Arc::new(Mutex::new(())),
            materializer_state_identity,
            previous_command_data: LockedPreviousCommandData::new(),
            incremental_db_state,
            persisted_dep_file_cache,
            dep_file_cache,
            paranoid,
            re_client_manager,
            blocking_executor,
            yakconfig_metadata: parse_yakconfig_metadata(root_config),
            tags,
            system_warning_config: SystemWarningConfig::from_config(root_config)?,
            clean_scratch_on_idle: root_config
                .parse::<RolloutPercentage>(YakconfigKeyRef {
                    section: "yak",
                    property: "clean_scratch_on_idle",
                })?
                .unwrap_or_else(RolloutPercentage::never)
                .roll(),
            page_out_on_idle,
        });

        Ok(Arc::new(Self { repo, dice_manager }))
    }

    pub async fn spawn_dice_dump(
        &self,
        path: &Path,
        format: DiceDumpFormat,
    ) -> yak_error::Result<()> {
        crate::daemon::dice_dump::dice_dump_spawn(self.dice_manager.unsafe_dice(), path, format)
            .await
    }
}

impl RepoState {
    fn create_materializer(
        fs: ProjectRoot,
        digest_config: DigestConfig,
        yak_out_path: ProjectRelativePathBuf,
        re_client_manager: Arc<ReConnectionManager>,
        blocking_executor: Arc<dyn BlockingExecutor>,
        deferred_materializer_configs: DeferredMaterializerConfigs,
        materializer_db: Option<MaterializerStateSqliteDb>,
        materializer_state: Option<MaterializerState>,
        daemon_dispatcher: EventDispatcher,
    ) -> yak_error::Result<Arc<dyn Materializer>> {
        Ok(Arc::new(DeferredMaterializer::new(
            fs,
            digest_config,
            yak_out_path,
            re_client_manager,
            blocking_executor,
            deferred_materializer_configs,
            materializer_db,
            materializer_state,
            daemon_dispatcher,
        )?))
    }
}

/// Tenant states known to this daemon.
///
/// The registry initially contains the tenant that started the daemon. Clients without an explicit
/// tenant identity continue to use that initial tenant for protocol compatibility.
#[derive(Allocative)]
struct TenantStateRegistry {
    initial_tenant: TenantKey,
    /// Stable handle for legacy single-repo callers. Registry entries are never replaced after
    /// insertion, and the same allocation is accounted for through `tenants`.
    #[allocative(skip)]
    initial_state: Arc<TenantState>,
    tenants: StdMutex<StdYakHashMap<TenantKey, Arc<TenantStateSlot<TenantState>>>>,
}

struct TenantStateSlot<T: Allocative> {
    spec: TenantSpec,
    state: OnceCell<Arc<T>>,
}

impl<T: Allocative> Allocative for TenantStateSlot<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut allocative::Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(allocative::Key::new("spec"), &self.spec);
        if let Some(state) = self.state.get() {
            visitor.visit_field(allocative::Key::new("state"), state);
        }
        visitor.exit();
    }
}

impl<T: Allocative> TenantStateSlot<T> {
    fn new(spec: TenantSpec) -> Self {
        Self {
            spec,
            state: OnceCell::new(),
        }
    }

    async fn get_or_try_init<F, Fut>(&self, init: F) -> yak_error::Result<Arc<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = yak_error::Result<Arc<T>>>,
    {
        self.state.get_or_try_init(init).await.map(Arc::clone)
    }

    fn is_initialized(&self) -> bool {
        self.state.get().is_some()
    }
}

impl TenantStateRegistry {
    async fn new(initial_tenant: Arc<TenantState>) -> yak_error::Result<Self> {
        let spec = TenantSpec::from_tenant_paths(&initial_tenant.repo.paths);
        let initial_key = spec.key().clone();
        let registry = Self {
            initial_tenant: initial_key,
            initial_state: initial_tenant.dupe(),
            tenants: StdMutex::new(StdYakHashMap::default()),
        };
        registry
            .get_or_create(spec, || async { Ok(initial_tenant) })
            .await?;
        Ok(registry)
    }

    async fn get_or_create<F, Fut>(
        &self,
        spec: TenantSpec,
        create: F,
    ) -> yak_error::Result<Arc<TenantState>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = yak_error::Result<Arc<TenantState>>>,
    {
        let key = spec.key().clone();
        let requested_spec = spec.clone();
        let entry = {
            let mut tenants = self
                .tenants
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            tenants
                .entry(key.clone())
                .or_insert_with(|| Arc::new(TenantStateSlot::new(spec)))
                .clone()
        };
        if entry.spec != requested_spec {
            return Err(yak_error!(
                yak_error::ErrorTag::Input,
                "Tenant key `{:?}` was requested with conflicting specifications",
                key
            ));
        }

        entry
            .get_or_try_init(|| async {
                let state = create().await?;
                let actual_key = TenantKey::from_tenant_paths(&state.repo.paths);
                if actual_key != key {
                    return Err(yak_error!(
                        yak_error::ErrorTag::Input,
                        "Constructed tenant key `{:?}` did not match requested key `{:?}`",
                        actual_key,
                        key
                    ));
                }
                Ok(state)
            })
            .await
    }

    fn initial_tenant(&self) -> Arc<TenantState> {
        self.initial_state.dupe()
    }

    fn legacy_tenant(&self) -> Option<Arc<TenantState>> {
        let tenants = self
            .tenants
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(
            tenants
                .get(&self.initial_tenant)
                .is_some_and(|entry| entry.is_initialized())
        );
        let initialized_tenant_count = tenants
            .values()
            .filter(|entry| entry.is_initialized())
            .count();
        (initialized_tenant_count == 1).then(|| self.initial_state.dupe())
    }
}

fn tenant_paths_from_client_context(
    client_context: &ClientContext,
) -> yak_error::Result<Option<TenantPaths>> {
    let Some(identity) = &client_context.tenant_identity else {
        return Ok(None);
    };

    let project_root = AbsNormPathBuf::try_from(identity.project_root.clone())
        .yak_error_context("Invalid tenant project root in client context")?;
    let isolation = FileNameBuf::try_from(identity.isolation.clone())
        .yak_error_context("Invalid tenant isolation in client context")?;
    Ok(Some(TenantPaths::new(
        ProjectRoot::new_unchecked(project_root),
        isolation,
    )))
}

/// DaemonStateData is the main shared data across all commands and repos. It's lazily initialized
/// on the first command that requires it.
#[derive(Allocative)]
pub struct DaemonStateData {
    tenants: TenantStateRegistry,
    tenant_state_factory: TenantStateFactory,

    /// Daemon-wide scheduling resources for repo-scoped blocking executors.
    pub blocking_executor_factory: Arc<BlockingExecutorFactory>,

    pub(crate) forkserver: ForkserverAccess,

    pub start_time: Instant,

    /// Http client used for materializer and RunAction implementations.
    pub http_client: HttpClient,

    /// Spawner
    pub spawner: Arc<YakSpawner>,

    /// Tracks memory usage. Used to make scheduling decisions.
    #[allocative(skip)]
    pub memory_tracker: Option<MemoryTrackerHandle>,

    /// A unique identifier for this instance of the daemon
    pub daemon_id: DaemonId,

    /// Cgroup path of the process that launched this daemon before yak moved the daemon into its
    /// managed cgroup.
    pub daemon_originating_cgroup: Option<String>,

    /// Semaphores for running actions locally. These need to be shared across commands.
    #[allocative(skip)]
    pub named_semaphores_for_run_actions: Arc<NamedSemaphores>,
}

impl DaemonStateData {
    fn repo_shared_services(&self) -> DaemonSharedServices<'_> {
        DaemonSharedServices {
            blocking_executor_factory: &self.blocking_executor_factory,
            memory_tracker: self.memory_tracker.as_ref(),
            daemon_id: &self.daemon_id,
        }
    }

    /// Select or initialize the tenant addressed by a client command.
    pub async fn tenant_for_client_context(
        &self,
        client_context: &ClientContext,
    ) -> yak_error::Result<Arc<TenantState>> {
        let Some(paths) = tenant_paths_from_client_context(client_context)? else {
            return self.tenant_for_legacy_client();
        };
        let spec = TenantSpec::from_tenant_paths(&paths);

        self.tenants
            .get_or_create(spec, || {
                self.tenant_state_factory
                    .create(paths, self.repo_shared_services())
            })
            .await
    }

    /// Select a tenant for an RPC added before requests carried a client context.
    pub async fn tenant_for_optional_client_context(
        &self,
        client_context: Option<&ClientContext>,
    ) -> yak_error::Result<Arc<TenantState>> {
        match client_context {
            Some(client_context) => self.tenant_for_client_context(client_context).await,
            None => self.tenant_for_legacy_client(),
        }
    }

    fn tenant_for_legacy_client(&self) -> yak_error::Result<Arc<TenantState>> {
        self.tenants.legacy_tenant().ok_or_else(|| {
            yak_error!(
                ErrorTag::Input,
                "Client did not provide a tenant identity after the daemon began serving multiple tenants"
            )
        })
    }

    /// The initial tenant for daemon-scoped operations whose protocol has no tenant identity.
    pub fn initial_tenant(&self) -> Arc<TenantState> {
        self.tenants.initial_tenant()
    }
}

impl DaemonState {
    pub(crate) async fn new(
        paths: InvocationPaths,
        init_ctx: YakdServerInitPreferences,
        rt: &Handle,
        final_artifact_materialization: FinalArtifactMaterialization,
        working_directory: WorkingDirectory,
        cgroup_tree: Option<YakCgroupTree>,
        daemon_id: DaemonId,
    ) -> Result<Self, yak_error::Error> {
        let data = Self::init_data(
            paths,
            init_ctx,
            rt,
            final_artifact_materialization,
            cgroup_tree,
            daemon_id,
        )
        .await
        .map_err(|e| {
            e.context("Error initializing DaemonStateData")
                .tag([ErrorTag::DaemonStateInitFailed])
        })?;

        tracing::info!("Daemon state is ready.");

        let state = DaemonState {
            data,
            working_directory,
        };
        Ok(state)
    }

    // Creates the initial DaemonStateData.
    // Starts up the watchman query.
    async fn init_data(
        paths: InvocationPaths,
        init_ctx: YakdServerInitPreferences,
        rt: &Handle,
        final_artifact_materialization: FinalArtifactMaterialization,
        cgroup_tree: Option<YakCgroupTree>,
        daemon_id: DaemonId,
    ) -> yak_error::Result<Arc<DaemonStateData>> {
        if yak_env!("YAK_TEST_INIT_DAEMON_ERROR", bool, applicability = testing)? {
            // TODO(minglunli): Errors here don't actually make it to invocation records which should be fixed
            return Err(yak_error::yak_error!(
                ErrorTag::TestOnly,
                "Injected init daemon error"
            ));
        }

        let daemon_state_data_rt = rt.clone();
        // Owned, because tenant construction happens in the spawned initialization future.
        let tenant_state_rt = rt.clone();
        let init_fut = async move {
            let invocation_paths = paths;
            let paths = invocation_paths.tenant_paths();
            let fs = paths.project_root().clone();

            tracing::info!("Reading config...");
            let legacy_cells = YakConfigBasedCells::parse_with_config_args(&fs, &[]).await?;

            tracing::info!("Starting...");

            let cells = &legacy_cells.cell_resolver;
            let root_config = &legacy_cells
                .parse_single_cell(cells.root_cell(), &fs)
                .await?;

            let blocking_executor_factory = Arc::new(BlockingExecutorFactory::create()?);

            let http_client = http_client_from_startup_config(&init_ctx.daemon_startup_config)
                .await
                .yak_error_context("Error creating HTTP client")?
                .build();

            tracing::info!("Creating memory tracker...");
            let memory_tracker = memory_tracker::create_memory_tracker(
                cgroup_tree,
                &init_ctx.daemon_startup_config.resource_control,
                &daemon_id,
            )
            .await?;

            // The forkserver creates its state directory recursively, so it does not depend on
            // the materializer creating yak-out first. One daemon-global forkserver serves every
            // repo; requests carry an absolute cwd.
            tracing::info!("Launching forkserver...");
            let forkserver = maybe_launch_forkserver(
                root_config,
                &invocation_paths.forkserver_state_dir(),
                memory_tracker.as_ref().map(|m| &m.cgroup_tree),
                &invocation_paths.isolation,
            )
            .await?;

            let (init_ctx, daemon_originating_cgroup) = init_ctx.split();
            let tenant_state_factory = TenantStateFactory {
                init_ctx,
                final_artifact_materialization,
                runtime: tenant_state_rt,
            };
            let tenant = tenant_state_factory
                .create_with_loaded_config(
                    paths,
                    &legacy_cells,
                    root_config,
                    DaemonSharedServices {
                        blocking_executor_factory: &blocking_executor_factory,
                        memory_tracker: memory_tracker.as_ref(),
                        daemon_id: &daemon_id,
                    },
                )
                .await?;

            let tenants = TenantStateRegistry::new(tenant).await?;
            Ok(Arc::new(DaemonStateData {
                tenants,
                tenant_state_factory,
                blocking_executor_factory,
                forkserver,
                start_time: std::time::Instant::now(),
                http_client,
                spawner: Arc::new(YakSpawner::new(daemon_state_data_rt)),
                memory_tracker,
                daemon_id: daemon_id.dupe(),
                daemon_originating_cgroup,
                named_semaphores_for_run_actions: Arc::new(NamedSemaphores::new()),
            }))
        };
        let daemon_listener_span = tracing::Span::current();
        rt.spawn(init_fut.instrument(daemon_listener_span)).await?
    }

    /// Prepares an event stream for a request by bootstrapping an event source and EventDispatcher pair. The given
    /// EventDispatcher will log to the returned EventSource.
    pub fn prepare_events(&self, trace_id: TraceId) -> (ChannelEventSource, EventDispatcher) {
        let (events, sink) = yak_events::create_source_sink_pair();
        let dispatcher = EventDispatcher::new(trace_id, self.data.daemon_id.dupe(), sink);
        (events, dispatcher)
    }

    /// Prepares a ServerCommandContext for processing a complex command (that accesses the dice computation graph, for example).
    ///
    /// This initializes (if necessary) the shared daemon state and syncs the watchman query (to flush any recent filesystem events).
    pub async fn prepare_command(
        &self,
        tenant: Arc<TenantState>,
        dispatcher: EventDispatcher,
        drop_guard: ActiveCommandDropGuard,
    ) -> yak_error::Result<BaseServerCommandContext> {
        let data = self.data();
        let repo = &tenant.repo;

        dispatcher.instant_event(yak_data::RestartConfiguration {
            enable_restarter: repo.restart_daemon_on_error,
        });

        tag_result!(
            "working_dir_not_connected",
            check_working_dir::check_working_dir(),
            quiet: true,
            daemon_in_memory_state_is_corrupted: true,
        )?;

        self.validate_cwd()
            .yak_error_context("Error validating working directory")?;

        dispatcher.instant_event(yak_data::TagEvent {
            tags: repo.tags.clone(),
        });

        // Sync any FS changes and invalidate DICE state if necessary.
        repo.io.settle().await?;

        Ok(BaseServerCommandContext {
            events: dispatcher,
            tenant,
            daemon: data.dupe(),
            _drop_guard: drop_guard,
        })
    }

    pub fn data(&self) -> Arc<DaemonStateData> {
        self.data.dupe()
    }

    pub fn validate_cwd(&self) -> yak_error::Result<()> {
        let res = self.working_directory.is_stale().and_then(|stale| {
            if stale {
                Err(yak_error!(
                    yak_error::ErrorTag::DaemonStaleWorkingDir,
                    "yak appears to be running in a stale working directory. \
                     This will likely lead to failed or slow builds. \
                     To remediate, restart yak."
                ))
            } else {
                Ok(())
            }
        });

        tag_result!(
            "stale_cwd",
            res,
            quiet: true,
            daemon_in_memory_state_is_corrupted: true,
        )?;

        Ok(())
    }
}

fn convert_algorithm_kind(kind: DigestAlgorithmFamily) -> yak_error::Result<DigestAlgorithm> {
    yak_error::Ok(match kind {
        DigestAlgorithmFamily::Sha1 => DigestAlgorithm::Sha1,
        DigestAlgorithmFamily::Sha256 => DigestAlgorithm::Sha256,
        DigestAlgorithmFamily::Blake3 => DigestAlgorithm::Blake3,
        DigestAlgorithmFamily::Blake3Keyed => {
            // Keyed BLAKE3 needs a key, and no yakconfig provides one yet.
            return Err(yak_error::yak_error!(
                yak_error::ErrorTag::Input,
                "{} is not supported",
                kind
            ));
        }
    })
}

/// Sensible defaults for http client when building from a DaemonStartupConfig.
const DEFAULT_MAX_REDIRECTS: usize = 10;
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5000;
const DEFAULT_READ_TIMEOUT_MS: u64 = 10000;

/// Customize an http client based on http.* legacy yakconfigs.
async fn http_client_from_startup_config(
    config: &DaemonStartupConfig,
) -> yak_error::Result<HttpClientBuilder> {
    let mut builder = HttpClientBuilder::https_with_system_roots_and_proxy_from_env().await?;
    builder.with_max_redirects(config.http.max_redirects.unwrap_or(DEFAULT_MAX_REDIRECTS));
    builder.with_http2(config.http.http2);
    builder.with_max_concurrent_requests(config.http.max_concurrent_requests);

    match config.http.connect_timeout() {
        Timeout::Value(d) => {
            builder.with_connect_timeout(Some(d));
        }
        Timeout::Default => {
            builder.with_connect_timeout(Some(Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS)));
        }
        _ => {}
    }
    match config.http.read_timeout() {
        Timeout::Value(d) => {
            builder.with_read_timeout(Some(d));
        }
        Timeout::Default => {
            builder.with_read_timeout(Some(Duration::from_millis(DEFAULT_READ_TIMEOUT_MS)));
        }
        _ => {}
    }
    match config.http.write_timeout() {
        Timeout::Value(d) => {
            builder.with_write_timeout(Some(d));
        }
        Timeout::Default | Timeout::NoTimeout => {}
    }

    Ok(builder)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use indoc::indoc;
    use yak_cli_proto::TenantIdentity;
    use yak_common::legacy_configs::configs::testing::parse;
    use yak_common::settings::YakSettings;
    use yak_fs::paths::abs_norm_path::AbsNormPathBuf;
    use yak_fs::paths::file_name::FileNameBuf;

    use super::*;

    fn tenant_spec() -> TenantSpec {
        let project_root = if cfg!(windows) {
            "C:\\project"
        } else {
            "/project"
        };
        TenantSpec::from_tenant_paths(&TenantPaths::new(
            ProjectRoot::new_unchecked(
                AbsNormPathBuf::try_from(project_root.to_owned())
                    .expect("test project root should be absolute and normalized"),
            ),
            FileNameBuf::try_from("v2".to_owned()).expect("test isolation should be a file name"),
        ))
    }

    #[tokio::test]
    async fn tenant_entry_initializes_once() -> yak_error::Result<()> {
        let entry = TenantStateSlot::<usize>::new(tenant_spec());
        let init_count = AtomicUsize::new(0);
        let first = entry.get_or_try_init(|| async {
            init_count.fetch_add(1, Ordering::Relaxed);
            tokio::task::yield_now().await;
            Ok(Arc::new(1))
        });
        let second = entry.get_or_try_init(|| async {
            init_count.fetch_add(1, Ordering::Relaxed);
            Ok(Arc::new(2))
        });

        let (first, second) = tokio::join!(first, second);
        let first = first?;
        let second = second?;
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(init_count.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[tokio::test]
    async fn tenant_entry_retries_failed_initialization() -> yak_error::Result<()> {
        let entry = TenantStateSlot::<usize>::new(tenant_spec());
        let first = entry
            .get_or_try_init(|| async {
                Err(yak_error!(
                    yak_error::ErrorTag::Input,
                    "injected initialization failure"
                ))
            })
            .await;
        assert!(first.is_err());
        assert!(!entry.is_initialized());

        let state = entry.get_or_try_init(|| async { Ok(Arc::new(1)) }).await?;
        assert_eq!(*state, 1);
        assert!(entry.is_initialized());
        Ok(())
    }

    #[test]
    fn tenant_paths_from_client_identity() -> yak_error::Result<()> {
        let project_root = if cfg!(windows) {
            "C:\\project"
        } else {
            "/project"
        };
        let client_context = ClientContext {
            tenant_identity: Some(TenantIdentity {
                project_root: project_root.to_owned(),
                isolation: "v2".to_owned(),
            }),
            ..Default::default()
        };

        let paths = tenant_paths_from_client_context(&client_context)?
            .expect("the client supplied a tenant identity");
        assert_eq!(paths.project_root().to_string(), project_root);
        assert_eq!(paths.isolation().as_str(), "v2");
        Ok(())
    }

    #[test]
    fn missing_client_identity_uses_compatibility_fallback() -> yak_error::Result<()> {
        assert!(tenant_paths_from_client_context(&ClientContext::default())?.is_none());
        Ok(())
    }

    #[test]
    fn invalid_client_project_root_is_rejected() {
        let client_context = ClientContext {
            tenant_identity: Some(TenantIdentity {
                project_root: "relative".to_owned(),
                isolation: "v2".to_owned(),
            }),
            ..Default::default()
        };

        assert!(tenant_paths_from_client_context(&client_context).is_err());
    }

    #[test]
    fn invalid_client_isolation_is_rejected() {
        let project_root = if cfg!(windows) {
            "C:\\project"
        } else {
            "/project"
        };
        let client_context = ClientContext {
            tenant_identity: Some(TenantIdentity {
                project_root: project_root.to_owned(),
                isolation: "nested/isolation".to_owned(),
            }),
            ..Default::default()
        };

        assert!(tenant_paths_from_client_context(&client_context).is_err());
    }

    #[tokio::test]
    async fn test_from_startup_config_defaults() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let builder =
            http_client_from_startup_config(&DaemonStartupConfig::testing_empty()).await?;
        assert_eq!(DEFAULT_MAX_REDIRECTS, builder.max_redirects().unwrap());
        assert_eq!(
            Some(Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS)),
            builder.connect_timeout()
        );
        assert_eq!(
            Some(Duration::from_millis(DEFAULT_READ_TIMEOUT_MS)),
            builder.read_timeout()
        );
        assert_eq!(None, builder.write_timeout());

        Ok(())
    }

    #[tokio::test]
    async fn test_from_startup_config_overrides() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let config = parse(
            &[(
                "config",
                indoc!(
                    r#"
                    [http]
                    max_redirects = 5
                    connect_timeout_ms = 10
                    write_timeout_ms = 5
                    "#
                ),
            )],
            "config",
        )?;
        let startup_config = DaemonStartupConfig::new(&config, &YakSettings::empty(), false)?;
        let builder = http_client_from_startup_config(&startup_config).await?;
        assert_eq!(5, builder.max_redirects().unwrap());
        assert_eq!(Some(Duration::from_millis(10)), builder.connect_timeout());
        assert_eq!(
            Some(Duration::from_millis(DEFAULT_READ_TIMEOUT_MS)),
            builder.read_timeout()
        );
        assert_eq!(Some(Duration::from_millis(5)), builder.write_timeout());

        Ok(())
    }

    #[tokio::test]
    async fn test_from_startup_config_zero_for_unset() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let config = parse(
            &[(
                "config",
                indoc!(
                    r#"
                    [http]
                    connect_timeout_ms = 0
                    "#,
                ),
            )],
            "config",
        )?;
        let startup_config = DaemonStartupConfig::new(&config, &YakSettings::empty(), false)?;
        let builder = http_client_from_startup_config(&startup_config).await?;
        assert_eq!(None, builder.connect_timeout());
        assert_eq!(
            Some(Duration::from_millis(DEFAULT_READ_TIMEOUT_MS)),
            builder.read_timeout()
        );
        assert_eq!(None, builder.write_timeout());

        Ok(())
    }
}
