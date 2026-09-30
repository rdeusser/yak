/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::time::Instant;

use allocative::Allocative;
use anyhow::Context;
use dupe::Dupe;
use either::Either;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::BoxStream;
use itertools::Itertools;
use prost::Message;
use remote_execution as RE;
use remote_execution::ActionResultRequest;
use remote_execution::ActionResultResponse;
use remote_execution::DownloadRequest;
use remote_execution::ExecuteRequest;
use remote_execution::ExecuteWithProgressResponse;
use remote_execution::ExtendDigestsTtlRequest;
use remote_execution::GetDigestsTtlRequest;
use remote_execution::GetDigestsTtlResponse;
use remote_execution::InlinedBlobWithDigest;
use remote_execution::NamedDigest;
use remote_execution::NamedDigestWithPermissions;
use remote_execution::OperationMetadata;
use remote_execution::REClient;
use remote_execution::REClientBuilder;
use remote_execution::RemoteExecutionMetadata;
use remote_execution::Stage;
use remote_execution::TActionResult2;
use remote_execution::TClientContextMetadata;
use remote_execution::TCode;
use remote_execution::TDigest;
use remote_execution::TExecutionPolicy;
use remote_execution::THostResourceRequirements;
use remote_execution::THostRuntimeRequirements;
use remote_execution::TLocalCacheStats;
use remote_execution::TaskState;
use remote_execution::UploadRequest;
use remote_execution::WriteActionResultRequest;
use remote_execution::WriteActionResultResponse;
use remote_execution::YakInfo;
use tokio::sync::Semaphore;
use yak_core::execution_types::executor_config::RemoteExecutorUseCase;
use yak_core::fs::project::ProjectRoot;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::yak_env;
use yak_data::ReQueueCancelled;
use yak_data::ReQueueNoWorkerAvailable;
use yak_data::ReQueueOverQuota;
use yak_error::YakErrorContext;
use yak_error::YakErrorOptionContext;
use yak_error::conversion::from_any_with_tag;
use yak_error::yak_error;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_hash::YakMutMap;
use yak_re_configuration::RemoteExecutionStaticMetadataImpl;

use crate::digest::CasDigestToReExt;
use crate::digest_config::DigestConfig;
use crate::directory::ActionImmutableDirectory;
use crate::execute::action_digest::ActionDigest;
use crate::execute::blobs::ActionBlobs;
use crate::execute::executor_stage_async;
use crate::execute::manager::CommandExecutionManager;
use crate::knobs::ExecutorGlobalKnobs;
use crate::materialize::materializer::Materializer;
use crate::re::action_identity::ReActionIdentity;
use crate::re::convert::platform_to_proto;
use crate::re::error::RemoteExecutionError;
use crate::re::error::test_re_error;
use crate::re::error::test_re_error_with_group;
use crate::re::error::with_error_handler;
use crate::re::manager::RemoteExecutionConfig;
use crate::re::metadata::RemoteExecutionMetadataExt;
use crate::re::queue_stats::QueueStats;
use crate::re::remote_action_result::ExecuteResponseWithQueueStats;
use crate::re::stats::LocalCacheRemoteExecutionClientStats;
use crate::re::stats::LocalCacheStats;
use crate::re::stats::OpStats;
use crate::re::stats::RemoteExecutionClientOpStats;
use crate::re::stats::RemoteExecutionClientStats;
use crate::re::ttl::re_expiration_from_ttl;
use crate::re::uploader::UploadStats;
use crate::re::uploader::Uploader;

pub enum ActionCacheWriteType {
    LocalCacheUpload,
    PermissionCheck,
    RemoteDepFile,
}
impl ActionCacheWriteType {
    fn as_str(&self) -> &'static str {
        match *self {
            ActionCacheWriteType::LocalCacheUpload => "local_cache_upload",
            ActionCacheWriteType::PermissionCheck => "permission_check",
            ActionCacheWriteType::RemoteDepFile => "remote_dep_file",
        }
    }
}

pub enum CancellationReason {
    NotSpecified,
    ReQueueTimeout,
}

#[derive(Default)]
pub struct Cancelled {
    pub reason: Option<CancellationReason>,
}

#[derive(Clone, Dupe, Allocative)]
pub struct RemoteExecutionClient {
    data: Arc<RemoteExecutionClientData>,
}

pub enum ExecutionStarted {
    Yes,
    No,
}

// The large one is the actual default case
#[allow(clippy::large_enum_variant)]
pub enum ExecuteResponseOrCancelled {
    Response(ExecuteResponseWithQueueStats),
    Cancelled(Cancelled, QueueStats, ExecutionStarted),
}

#[derive(Allocative)]
struct RemoteExecutionClientData {
    client: RemoteExecutionClientImpl,
    uploads: OpStats,
    downloads: OpStats,
    action_cache: OpStats,
    executes: OpStats,
    materializes: OpStats,
    write_action_results: OpStats,
    get_digests_ttl: OpStats,
    extend_digest_ttl: OpStats,
    local_cache: LocalCacheStats,
}

impl RemoteExecutionClient {
    pub async fn new(re_config: &RemoteExecutionConfig) -> yak_error::Result<Self> {
        if yak_env!("YAK_TEST_FAIL_CONNECT", bool, applicability = testing)? {
            return Err(yak_error!(
                yak_error::ErrorTag::Input,
                "Injected RE Connection error"
            ));
        }

        // Creating the client normally takes seconds. Every command sharing the connection waits
        // on it, so an attempt that wedges must fail rather than hang them all. 0 removes the bound.
        let timeout_s = yak_env!("YAK_RE_CONNECT_TIMEOUT_S", type = u64, default = 120)?;
        let create = RemoteExecutionClientImpl::new(re_config);
        let client = if timeout_s == 0 {
            create.await?
        } else {
            tokio::time::timeout(Duration::from_secs(timeout_s), create)
                .await
                .map_err(|_| {
                    yak_error!(
                        yak_error::ErrorTag::ReDeadlineExceeded,
                        "Creating the RE client did not finish within {}s. If this persists, run `yak kill`",
                        timeout_s
                    )
                })??
        };

        Ok(Self {
            data: Arc::new(RemoteExecutionClientData {
                client,
                uploads: OpStats::default(),
                downloads: OpStats::default(),
                action_cache: OpStats::default(),
                executes: OpStats::default(),
                materializes: OpStats::default(),
                write_action_results: OpStats::default(),
                get_digests_ttl: OpStats::default(),
                extend_digest_ttl: OpStats::default(),
                local_cache: Default::default(),
            }),
        })
    }

    pub async fn new_retry(re_config: &RemoteExecutionConfig) -> yak_error::Result<Self> {
        // Loop happens times-1 times at most
        for i in 1..re_config.connection_retries {
            match Self::new(re_config).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if e.find_typed_context::<RemoteExecutionError>().is_none() {
                        // If we cannot connect to RE due to some non-RE error, we should not retry
                        // And should just return the error immediately as it's unlikely to be flakey
                        return Err(e);
                    }

                    tracing::warn!(
                        "Failed to connect to RE, retrying after sleeping {} seconds: {:#?}",
                        i,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(i as u64)).await;
                }
            }
        }
        Self::new(re_config).await
    }

    pub async fn action_cache(
        &self,
        action_digest: ActionDigest,
        metadata: &RemoteExecutionMetadata,
        use_case: RemoteExecutorUseCase,
        platform: &RE::Platform,
    ) -> yak_error::Result<Option<ActionResultResponse>> {
        let _ac = self.data.client.action_cache_semaphore.acquire().await;

        self.data
            .action_cache
            .op(self
                .data
                .client
                .action_cache(action_digest, metadata, use_case, platform))
            .await
    }

    pub async fn upload(
        &self,
        fs: &ProjectRoot,
        materializer: &dyn Materializer,
        blobs: &ActionBlobs,
        dir_path: &ProjectRelativePath,
        input_dir: &ActionImmutableDirectory,
        use_case: RemoteExecutorUseCase,
        identity: Option<&ReActionIdentity<'_>>,
        digest_config: DigestConfig,
        deduplicate_get_digests_ttl_calls: bool,
    ) -> yak_error::Result<UploadStats> {
        // Actually upload to CAS
        let _cas = self.data.client.cas_semaphore.acquire().await;

        self.data
            .uploads
            .op(Uploader::upload(
                fs,
                self,
                materializer,
                dir_path,
                input_dir,
                blobs,
                use_case,
                identity,
                digest_config,
                deduplicate_get_digests_ttl_calls,
            ))
            .await
    }

    pub async fn upload_files_and_directories(
        &self,
        files_with_digest: Vec<NamedDigest>,
        directories: Vec<remote_execution::Path>,
        inlined_blobs_with_digest: Vec<InlinedBlobWithDigest>,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<()> {
        self.data
            .uploads
            .op(self.data.client.upload_files_and_directories(
                files_with_digest,
                directories,
                inlined_blobs_with_digest,
                metadata,
            ))
            .await
    }

    pub async fn execute(
        &self,
        action_digest: ActionDigest,
        platform: &RE::Platform,
        use_case: RemoteExecutorUseCase,
        identity: &ReActionIdentity<'_>,
        manager: &mut CommandExecutionManager,
        skip_cache_read: bool,
        skip_cache_write: bool,
        re_max_queue_time: Option<Duration>,
        re_resource_units: Option<i64>,
        knobs: &ExecutorGlobalKnobs,
        worker_tool_action_digest: Option<ActionDigest>,
        priority: Option<i32>,
    ) -> yak_error::Result<ExecuteResponseOrCancelled> {
        self.data
            .executes
            .op(self.data.client.execute(
                action_digest,
                platform,
                use_case,
                identity,
                manager,
                skip_cache_read,
                skip_cache_write,
                re_max_queue_time,
                re_resource_units,
                knobs,
                worker_tool_action_digest,
                priority,
            ))
            .await
    }

    pub async fn materialize_files(
        &self,
        files: Vec<NamedDigestWithPermissions>,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<()> {
        let stat = self
            .data
            .materializes
            .op(self.data.client.materialize_files(files, metadata))
            .await?;
        self.data.local_cache.update(&stat);
        Ok(())
    }

    pub async fn download_typed_blobs<T: Message + Default>(
        &self,
        identity: Option<&ReActionIdentity<'_>>,
        digests: Vec<TDigest>,
        use_case: RemoteExecutorUseCase,
    ) -> yak_error::Result<Vec<T>> {
        self.data
            .downloads
            .op(self
                .data
                .client
                .download_typed_blobs(identity, digests, use_case))
            .await
            .map(|r| {
                self.data.local_cache.update(&r.1);
                r.0
            })
    }

    pub async fn download_blob(
        &self,
        digest: &TDigest,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<Vec<u8>> {
        self.data
            .downloads
            .op(self.data.client.download_blob(digest, metadata))
            .await
            .map(|r| {
                self.data.local_cache.update(&r.1);
                r.0
            })
    }

    pub async fn upload_blob(
        &self,
        blob: InlinedBlobWithDigest,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<TDigest> {
        self.data
            .uploads
            .op(self.data.client.upload_blob(blob, metadata))
            .await
    }

    pub async fn get_digests_ttl(
        &self,
        digests: Vec<TDigest>,
        metadata: &RemoteExecutionMetadata,
        is_for_upload: bool,
    ) -> yak_error::Result<GetDigestsTtlResponse> {
        self.data
            .get_digests_ttl
            .op(self
                .data
                .client
                .get_digests_ttl(digests, metadata, is_for_upload))
            .await
    }

    pub async fn get_digest_expirations(
        &self,
        digests: Vec<TDigest>,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<Vec<(TDigest, jiff::Timestamp)>> {
        let now = jiff::Timestamp::now();
        let ttls = self.get_digests_ttl(digests, metadata, false).await?;
        Ok(ttls
            .digests_with_ttl
            .into_iter()
            .map(|t| {
                let expiration = re_expiration_from_ttl(now, t.ttl, &t.digest);
                (t.digest, expiration)
            })
            .collect())
    }

    pub async fn extend_digest_ttl(
        &self,
        digests: Vec<TDigest>,
        ttl: Duration,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<()> {
        self.data
            .extend_digest_ttl
            .op(self.data.client.extend_digest_ttl(digests, ttl, metadata))
            .await
    }

    pub async fn write_action_result(
        &self,
        digest: ActionDigest,
        result: &mut TActionResult2,
        use_case: RemoteExecutorUseCase,
        platform: &RE::Platform,
        write_type: ActionCacheWriteType,
    ) -> yak_error::Result<WriteActionResultResponse> {
        let _ac = self.data.client.action_cache_semaphore.acquire().await;

        self.data
            .write_action_results
            .op(self
                .data
                .client
                .write_action_result(digest, result, use_case, platform, write_type))
            .await
    }

    pub fn get_session_id(&self) -> &str {
        self.data.client.get_session_id()
    }

    pub fn release_temporary_memory(&self) -> yak_error::Result<()> {
        self.data
            .client
            .client()
            .release_temporary_memory()
            .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::Tier0))
    }

    pub fn fill_network_stats(&self, stats: &mut RemoteExecutionClientStats) {
        stats.uploads = RemoteExecutionClientOpStats::from(&self.data.uploads);
        stats.downloads = RemoteExecutionClientOpStats::from(&self.data.downloads);
        stats.executes = RemoteExecutionClientOpStats::from(&self.data.executes);
        stats.action_cache = RemoteExecutionClientOpStats::from(&self.data.action_cache);
        stats.write_action_results =
            RemoteExecutionClientOpStats::from(&self.data.write_action_results);
        stats.materializes = RemoteExecutionClientOpStats::from(&self.data.materializes);
        stats.get_digest_expirations =
            RemoteExecutionClientOpStats::from(&self.data.get_digests_ttl);
        stats.local_cache = LocalCacheRemoteExecutionClientStats::from(&self.data.local_cache);
    }

    pub(super) fn get_raw_re_client(&self) -> &REClient {
        self.data.client.client()
    }
}

#[derive(Allocative)]
struct RemoteExecutionClientImpl {
    #[allocative(skip)]
    client: Option<REClient>,
    skip_remote_cache: bool,
    /// How many simultaneous requests to RE
    #[allocative(skip)]
    cas_semaphore: Arc<Semaphore>,
    /// Bounds concurrent action cache reads/writes. Particularly at build start thousands
    /// of actions become ready at once, and this prevents us from issuing requests
    /// faster than they can be filled, reducing the transient memory we have to hold.
    #[allocative(skip)]
    action_cache_semaphore: Arc<Semaphore>,
    /// How many simultaneous execute requests to RE
    #[allocative(skip)]
    exec_semaphore: Arc<Semaphore>,
    /// How many files we can be downloading concurrently.
    #[allocative(skip)]
    download_files_semapore: Arc<Semaphore>,
    /// How many files to kick off downloading concurrently for one request. This should be smaller
    /// than the files semaphore to ensure we can actually *acquire* that semaphore.
    download_chunk_size: usize,
}

fn anticipated_queue_duration(
    event: &ExecuteWithProgressResponse,
) -> anyhow::Result<Option<Duration>> {
    // Return a queue estimate even if RE dequeues immediately
    if let Some(duration) = yak_env!(
        "YAK_TEST_RE_QUEUE_ESTIMATE_S",
        type=u64,
        applicability = testing
    )
    // Stringify the error because we can't deal with yak_errors here
    .map_err(|e| anyhow::anyhow!(e))?
    {
        return Ok(Some(Duration::from_secs(duration)));
    }

    if let Some(info) = &event.metadata.task_info {
        // TODO make RE report same value as estimated_queue_time_ms, switch to that then stop reporting new_estimated_queue_time_ms
        let estimated_queue_time_ms = info.new_estimated_queue_time_ms;

        let est = u64::try_from(estimated_queue_time_ms)
            .context("estimated_queue_time_ms from RE is negative")?;
        return Ok(Some(Duration::from_millis(est)));
    }
    Ok(None)
}

// Debugging tool: A set of action digests to pretend that we got cache misses on regardless of the
// actual state of the remote cache
//
// After we execute an action once, we no longer want to pretend that we got cache misses on it if
// we execute it again (say, on a subsequent build); the `AtomicBool` in the value deals with that,
// it's true after the first time we execute the action
static INDUCED_CACHE_MISSES: LazyLock<Option<YakMutMap<String, AtomicBool>>> =
    LazyLock::new(|| {
        if let Ok(p) = std::env::var("YAK_INDUCED_CACHE_MISSES") {
            let c = fs_util::read_to_string(AbsNormPath::new(&p).unwrap())
                .categorize_input()
                .unwrap();
            Some(
                c.lines()
                    .map(|s| (s.to_owned(), AtomicBool::new(false)))
                    .collect(),
            )
        } else {
            None
        }
    });

impl RemoteExecutionClientImpl {
    async fn new(re_config: &RemoteExecutionConfig) -> yak_error::Result<Self> {
        let op_name = "REClientBuilder";
        tracing::info!("Creating a new RE client");

        let res: yak_error::Result<Self> = try {
            let download_concurrency =
                yak_env!("YAK_RE_DOWNLOAD_CONCURRENCY", type=usize, default=256)?;

            // Split things up into smaller chunks.
            let download_chunk_size = std::cmp::max(download_concurrency / 8, 1);
            let static_metadata = &re_config.static_metadata;

            let client = {
                with_error_handler(
                    op_name,
                    "<none>",
                    REClientBuilder::build_and_connect(&static_metadata.0).await,
                )?
            };

            Self {
                client: Some(client),
                skip_remote_cache: re_config.skip_remote_cache,
                cas_semaphore: Arc::new(Semaphore::new(static_metadata.cas_semaphore_size())),
                action_cache_semaphore: Arc::new(Semaphore::new(
                    static_metadata.action_cache_semaphore_size(),
                )),
                exec_semaphore: Arc::new(Semaphore::new(static_metadata.exec_semaphore_size())),
                download_files_semapore: Arc::new(Semaphore::new(download_concurrency)),
                download_chunk_size,
            }
        };

        res
    }

    fn client(&self) -> &REClient {
        self.client
            .as_ref()
            .expect("REClient is always present unless dropped")
    }

    fn get_session_id(&self) -> &str {
        self.client().get_session_id()
    }

    async fn action_cache(
        &self,
        action_digest: ActionDigest,
        metadata: &RemoteExecutionMetadata,
        use_case: RemoteExecutorUseCase,
        platform: &RE::Platform,
    ) -> yak_error::Result<Option<ActionResultResponse>> {
        if let Some(m) = &*INDUCED_CACHE_MISSES {
            if m.get(&action_digest.to_string())
                .is_some_and(|b| !b.load(std::sync::atomic::Ordering::Relaxed))
            {
                return Ok(None);
            }
        }

        let res = with_error_handler(
            "action_cache",
            self.get_session_id(),
            self.client()
                .get_action_cache_client()
                .get_action_result(
                    metadata,
                    ActionResultRequest {
                        digest: action_digest.to_re(),
                        platform: Some(platform.clone()),
                        ..Default::default()
                    },
                )
                .await,
        );

        let res = match res {
            Ok(r) => Some(r),
            Err(e) => match e.find_typed_context::<RemoteExecutionError>() {
                Some(re_err) if re_err.code == TCode::NOT_FOUND => None,
                _ => return Err(e),
            },
        };
        Ok(res)
    }

    async fn upload_files_and_directories(
        &self,
        files_with_digest: Vec<NamedDigest>,
        directories: Vec<remote_execution::Path>,
        inlined_blobs_with_digest: Vec<InlinedBlobWithDigest>,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<()> {
        let upload = self.client().get_cas_client().upload(
            metadata,
            UploadRequest {
                files_with_digest: Some(files_with_digest),
                inlined_blobs_with_digest: Some(inlined_blobs_with_digest),
                directories: Some(directories),
                upload_only_missing: true,
                ..Default::default()
            },
        );
        with_error_handler(
            "upload_files_and_directories",
            self.get_session_id(),
            upload.await,
        )?;
        Ok(())
    }

    async fn execute_impl(
        &self,
        metadata: &RemoteExecutionMetadata,
        request: ExecuteRequest,
        action_digest: &ActionDigest,
        manager: &mut CommandExecutionManager,
        re_max_queue_time: Option<Duration>,
        platform: &remote_execution::Platform,
        knobs: &ExecutorGlobalKnobs,
        persistent_worker: bool,
    ) -> anyhow::Result<ExecuteResponseOrCancelled> {
        use yak_data::ReAfterAction;
        use yak_data::ReBeforeAction;
        use yak_data::ReExecute;
        use yak_data::ReQueue;
        use yak_data::ReUnknown;
        use yak_data::ReWorkerDownload;
        use yak_data::ReWorkerUpload;
        use yak_data::re_stage;

        let action_key = if knobs.log_action_keys {
            metadata
                .action_history_info
                .as_ref()
                .map(|h| h.action_key.clone())
        } else {
            None
        };

        let re_use_case = metadata.use_case_id.clone();

        #[allow(clippy::large_enum_variant)]
        enum ResponseOrStateChange {
            Present(ExecuteWithProgressResponse),
            Cancelled(Cancelled),
        }

        /// Wait for either the ExecuteResponse to show up, or a stage change, within a span
        /// on the CommandExecutionManager.
        async fn wait_for_response_or_stage_change(
            receiver: &mut BoxStream<'static, anyhow::Result<ExecuteWithProgressResponse>>,
            previous_stage: Stage,
            previous_metadata: &Option<OperationMetadata>,
            report_stage: re_stage::Stage,
            manager: &mut CommandExecutionManager,
            queue_stats: &mut QueueStats,
            re_fallback_on_estimated_queue_time_exceeds_duration: Option<Duration>,
            re_cancel_on_estimated_queue_time_exceeds: Option<Duration>,
        ) -> anyhow::Result<ResponseOrStateChange> {
            executor_stage_async(
                yak_data::ReStage {
                    stage: Some(report_stage),
                },
                async move {
                    loop {
                        let now = Instant::now();

                        let event = {
                            let next = futures::future::select(
                                manager.inner.liveliness_observer.while_alive(),
                                receiver.next(),
                            )
                            .await;

                            // Whenever time elapsed and we were previously in
                            // queued stage, we add the time that elapsed. If we
                            // just exited a queued stage, then we tracked how long
                            // we spent queued. If we are still in queue, we'll keep
                            // counting. This might not be as accurate as the RE
                            // logging but at least it works even if we get cancelled.
                            if matches!(previous_stage, Stage::QUEUED) {
                                queue_stats.cumulative_queue_duration += Instant::now() - now;
                            }

                            match next {
                                futures::future::Either::Left((_dead, _)) => {
                                    return Ok(ResponseOrStateChange::Cancelled(Cancelled {
                                        ..Default::default()
                                    }));
                                }
                                futures::future::Either::Right((event, _)) => match event {
                                    Some(event) => event,
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "RE execution did not yield a ExecuteResponse"
                                        ));
                                    }
                                },
                            }
                            .context("Error was returned on the stream by RE")?
                        };

                        // Check if we have a response or if the stage has changed
                        let stage_changed = event.stage != previous_stage;

                        // Check if queue information has changed (when in QUEUED stage)
                        let queue_info_changed = if matches!(event.stage, Stage::QUEUED)
                            && matches!(previous_stage, Stage::QUEUED)
                        {
                            // Check if the kind of task state has changed.
                            let previous_task_info = previous_metadata
                                .as_ref()
                                .and_then(|m| m.task_info.as_ref());
                            match (&event.metadata.task_info, previous_task_info) {
                                (Some(current_task_info), Some(previous_task_info)) => {
                                    std::mem::discriminant(&current_task_info.state)
                                        != std::mem::discriminant(&previous_task_info.state)
                                }
                                (None, None) => false,
                                _ => true,
                            }
                        } else {
                            false
                        };

                        if event.execute_response.is_some() || stage_changed || queue_info_changed {
                            return Ok(ResponseOrStateChange::Present(event));
                        }

                        if let Some(anticipated_queue_duration) =
                            anticipated_queue_duration(&event)?
                        {
                            if let Some(re_queue_threshold) =
                                re_cancel_on_estimated_queue_time_exceeds
                            {
                                if anticipated_queue_duration > re_queue_threshold {
                                    return Ok(ResponseOrStateChange::Cancelled(Cancelled {
                                        reason: Some(CancellationReason::ReQueueTimeout),
                                    }));
                                }
                            }

                            if let Some(re_acceptable_anticipated_queue_duration) =
                                re_fallback_on_estimated_queue_time_exceeds_duration
                            {
                                if anticipated_queue_duration
                                    > re_acceptable_anticipated_queue_duration
                                {
                                    manager.on_result_delayed();
                                }
                            }
                        }
                    }
                },
            )
            .await
        }

        fn get_queue_state(
            action_digest: String,
            use_case: String,
            operation_metadata: &Option<OperationMetadata>,
        ) -> re_stage::Stage {
            let queue_info = ReQueue {
                action_digest,
                use_case,
            };
            let state = operation_metadata
                .as_ref()
                .and_then(|m| m.task_info.as_ref())
                .map(|i| &i.state);
            match state {
                // Waiting to run, no extra info needed
                Some(TaskState::enqueued(..)) => re_stage::Stage::Queue(queue_info),
                Some(TaskState::waiting_on_reservation(..)) => re_stage::Stage::Queue(queue_info),
                // Useful info to display
                Some(TaskState::no_worker_available(..)) => {
                    re_stage::Stage::QueueNoWorkerAvailable(ReQueueNoWorkerAvailable {
                        queue_info: Some(queue_info),
                    })
                }
                Some(TaskState::cancelled(..)) => {
                    re_stage::Stage::QueueCancelled(ReQueueCancelled {
                        queue_info: Some(queue_info),
                    })
                }
                Some(TaskState::over_quota(..)) => {
                    re_stage::Stage::QueueOverQuota(ReQueueOverQuota {
                        queue_info: Some(queue_info),
                    })
                }
                // Unknown
                Some(TaskState::UnknownField(..)) => re_stage::Stage::Queue(queue_info),
                None => re_stage::Stage::Queue(queue_info),
            }
        }

        fn re_stage_from_exe_stage(
            stage: Stage,
            operation_metadata: &Option<OperationMetadata>,
            action_digest: String,
            platform: &remote_execution::Platform,
            action_key: &Option<String>,
            use_case: String,
            persistent_worker: bool,
        ) -> re_stage::Stage {
            match stage {
                Stage::QUEUED => get_queue_state(action_digest, use_case, operation_metadata),
                Stage::MATERIALIZING_INPUT => re_stage::Stage::WorkerDownload(ReWorkerDownload {
                    action_digest,
                    use_case,
                }),
                Stage::BEFORE_ACTION => {
                    re_stage::Stage::BeforeActionExecution(ReBeforeAction { action_digest })
                }
                Stage::EXECUTING => re_stage::Stage::Execute(ReExecute {
                    action_digest,
                    platform: Some(platform_to_proto(platform)),
                    action_key: action_key.clone(),
                    use_case,
                    persistent_worker,
                }),
                Stage::AFTER_ACTION => {
                    re_stage::Stage::AfterActionExecution(ReAfterAction { action_digest })
                }
                Stage::UPLOADING_OUTPUT => re_stage::Stage::WorkerUpload(ReWorkerUpload {
                    action_digest,
                    use_case,
                }),
                _ => {
                    tracing::debug!(
                        "Received unexpected RE stage {:#?} for action: {}",
                        stage,
                        action_digest
                    );
                    re_stage::Stage::Unknown(ReUnknown { action_digest })
                }
            }
        }

        // There are four possible outcomes, three of them failures:
        // 1. We get Err, which means we failed to communicate to RE.
        // 2. We get Ok(error.code != TCode::Ok), which means we have an RE-level failure
        // 3. We get Ok(action_result.exit_code != 0), which means the command failed
        // 4. We get Ok(0 everywhere), which means it worked

        // Obtain a stream of events from RE. If this fails then that is case #1 above so we
        // bail.
        let mut receiver = self
            .client()
            .get_execution_client()
            .execute_with_progress(metadata, request)
            // boxed() to segment the future
            .boxed()
            .await
            .context("Failed to start remote execution")?;

        // Now we wait until the ExecuteResponse shows up, and produce events accordingly. If
        // this doesn't give us an ExecuteResponse then this is case #1 again so we also fail.
        let action_digest_str = action_digest.to_string();
        let mut queue_stats = QueueStats::default();
        let mut exe_stage = Stage::QUEUED;
        let mut execution_started = ExecutionStarted::No;
        let mut operation_metadata = None;

        let re_fallback_on_estimated_queue_time_exceeds = knobs
            .re_fallback_on_estimated_queue_time_exceeds
            .or(re_max_queue_time);

        loop {
            let progress_response = wait_for_response_or_stage_change(
                &mut receiver,
                exe_stage,
                &operation_metadata,
                re_stage_from_exe_stage(
                    exe_stage,
                    &operation_metadata,
                    action_digest_str.clone(),
                    platform,
                    &action_key,
                    re_use_case.clone(),
                    persistent_worker,
                ),
                manager,
                &mut queue_stats,
                re_fallback_on_estimated_queue_time_exceeds,
                knobs.re_cancel_on_estimated_queue_time_exceeds,
            )
            .await?;

            let progress_response = match progress_response {
                ResponseOrStateChange::Present(r) => r,
                ResponseOrStateChange::Cancelled(c) => {
                    return Ok(ExecuteResponseOrCancelled::Cancelled(
                        c,
                        queue_stats,
                        execution_started,
                    ));
                }
            };

            // Return the result if we're done
            if let Some(execute_response) = progress_response.execute_response {
                return Ok(ExecuteResponseOrCancelled::Response(
                    ExecuteResponseWithQueueStats {
                        execute_response,
                        queue_stats,
                    },
                ));
            }

            // Change the stage
            exe_stage = progress_response.stage;
            if exe_stage == Stage::EXECUTING {
                execution_started = ExecutionStarted::Yes;
            }
            operation_metadata = Some(progress_response.metadata);
        }
    }

    pub async fn execute(
        &self,
        action_digest: ActionDigest,
        platform: &RE::Platform,
        use_case: RemoteExecutorUseCase,
        identity: &ReActionIdentity<'_>,
        manager: &mut CommandExecutionManager,
        skip_cache_read: bool,
        skip_cache_write: bool,
        re_max_queue_time: Option<Duration>,
        re_resource_units: Option<i64>,
        knobs: &ExecutorGlobalKnobs,
        worker_tool_action_digest: Option<ActionDigest>,
        priority: Option<i32>,
    ) -> yak_error::Result<ExecuteResponseOrCancelled> {
        let _exec_permit = self.exec_semaphore.acquire().await;

        let _unused = worker_tool_action_digest;

        if yak_env!("YAK_TEST_FAIL_RE_EXECUTE", bool, applicability = testing)? {
            return Err(test_re_error("Injected error", TCode::FAILED_PRECONDITION));
        }

        if yak_env!(
            "YAK_TEST_FAIL_RE_RESOURCE_EXHAUSTED",
            bool,
            applicability = testing
        )? {
            return Err(test_re_error(
                "Injected RE resource exhausted error",
                TCode::RESOURCE_EXHAUSTED,
            ));
        }

        let induced_cache_miss = if let Some(m) = &*INDUCED_CACHE_MISSES {
            m.get(&action_digest.to_string())
                .filter(|v| !v.load(std::sync::atomic::Ordering::Relaxed))
        } else {
            None
        };

        let metadata = RemoteExecutionMetadata {
            platform: Some(platform.clone()),
            do_not_cache: skip_cache_write,
            yak_info: Some(YakInfo {
                version: yak_build_info::revision()
                    .map(|s| s.to_owned())
                    .unwrap_or_default(),
                build_id: identity.trace_id.to_string(),
                ..Default::default()
            }),
            ..use_case.metadata(Some(identity))
        };

        let request = ExecuteRequest {
            skip_cache_lookup: self.skip_remote_cache
                || skip_cache_read
                || induced_cache_miss.is_some(),
            execution_policy: Some(TExecutionPolicy {
                affinity_keys: vec![identity.affinity_key.clone()],
                priority: priority.unwrap_or_default(),
                ..Default::default()
            }),
            action_digest: action_digest.to_re(),
            host_runtime_requirements: THostRuntimeRequirements {
                platform: platform.clone(),
                host_resource_requirements: THostResourceRequirements {
                    input_files_bytes: identity.paths.input_files_bytes() as i64,
                    resource_units: re_resource_units.unwrap_or_default(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let re_action = format!("Execute with digest {}", action_digest);
        let res = with_error_handler(
            re_action.as_str(),
            self.get_session_id(),
            self.execute_impl(
                &metadata,
                request,
                &action_digest,
                manager,
                re_max_queue_time,
                platform,
                knobs,
                worker_tool_action_digest.is_some(),
            )
            .await,
        );

        if let Some(induced_cache_miss) = induced_cache_miss {
            induced_cache_miss.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        res
    }

    /// Fetches a list of digests from the CAS and casts them to Tree objects.
    /// If fetching or decoding fails for one or more digests, returns an Err.
    async fn download_typed_blobs<T: Message + Default>(
        &self,
        identity: Option<&ReActionIdentity<'_>>,
        digests: Vec<TDigest>,
        use_case: RemoteExecutorUseCase,
    ) -> yak_error::Result<(Vec<T>, TLocalCacheStats)> {
        if digests.is_empty() {
            return Ok((Vec::new(), TLocalCacheStats::default()));
        }
        if yak_env!("YAK_TEST_FAIL_RE_DOWNLOADS", bool, applicability = testing)? {
            return Err(test_re_error_with_group(
                "Injected error",
                TCode::NOT_FOUND,
                RE::TCodeReasonGroup::DIGEST_NOT_FOUND,
            ));
        }
        let expected_blobs = digests.len();
        let response = with_error_handler(
            "download_typed_blobs",
            self.get_session_id(),
            self.client()
                .get_cas_client()
                .download(
                    &use_case.metadata(identity),
                    DownloadRequest {
                        inlined_digests: Some(digests),
                        ..Default::default()
                    },
                )
                .await,
        )?;

        let mut blobs: Vec<T> = Vec::with_capacity(expected_blobs);
        if let Some(ds) = response.inlined_blobs {
            for d in ds {
                blobs.push(
                    Message::decode(d.blob.as_slice()).with_yak_error_context(|| {
                        format!("Failed to Protobuf decode tree at `{}`", d.digest)
                    })?,
                );
            }
        }

        // This shouldn't happen, but we can't just assume the CAS won't ever break
        if blobs.len() != expected_blobs {
            return Err(yak_error!(
                yak_error::ErrorTag::CasBlobCountMismatch,
                "CAS client returned fewer blobs than expected."
            ));
        }

        Ok((blobs, response.local_cache_stats))
    }

    pub async fn download_blob(
        &self,
        digest: &TDigest,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<(Vec<u8>, TLocalCacheStats)> {
        let re_action = format!("download_blob for digest {digest}");
        let response = with_error_handler(
            re_action.as_str(),
            self.get_session_id(),
            self.client()
                .get_cas_client()
                .download(
                    metadata,
                    DownloadRequest {
                        inlined_digests: Some(vec![digest.clone()]),
                        ..Default::default()
                    },
                )
                // boxed() to segment the future
                .boxed()
                .await,
        )?;

        response
            .inlined_blobs
            .into_iter()
            .flat_map(|blobs| blobs.into_iter())
            .next()
            .map(|blob| (blob.blob, response.local_cache_stats))
            .with_internal_error(|| format!("No digest was returned in request for {digest}"))
    }

    pub async fn upload_blob(
        &self,
        blob: InlinedBlobWithDigest,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<TDigest> {
        let upload = self
            .client()
            .upload_blob_with_digest(blob.blob, blob.digest, metadata);
        with_error_handler("upload_blob", self.get_session_id(), upload.await)
    }

    async fn materialize_files(
        &self,
        files: Vec<NamedDigestWithPermissions>,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<TLocalCacheStats> {
        if yak_env!("YAK_TEST_FAIL_RE_DOWNLOADS", bool, applicability = testing)? {
            return Err(test_re_error_with_group(
                "Injected error",
                TCode::NOT_FOUND,
                RE::TCodeReasonGroup::DIGEST_NOT_FOUND,
            ));
        }

        let futs = chunks(files, self.download_chunk_size).map(|chunk| async move {
            let _permit = self
                .download_files_semapore
                .acquire_many(
                    chunk
                        .len()
                        .try_into()
                        .yak_error_context("chunk is too large")?,
                )
                .await
                .yak_error_context("Failed to acquire download_files_semapore")?;

            let response = with_error_handler(
                "materialize_files",
                self.get_session_id(),
                self.client()
                    .get_cas_client()
                    .download(
                        metadata,
                        DownloadRequest {
                            file_digests: Some(chunk),
                            ..Default::default()
                        },
                    )
                    .await,
            )?;

            yak_error::Ok(response.local_cache_stats)
        });

        let stat = yak_util::future::try_join_all(futs).await?.iter().fold(
            TLocalCacheStats::default(),
            |acc, x| TLocalCacheStats {
                hits_files: acc.hits_files + x.hits_files,
                hits_bytes: acc.hits_bytes + x.hits_bytes,
                misses_files: acc.misses_files + x.misses_files,
                misses_bytes: acc.misses_bytes + x.misses_bytes,
                ..Default::default()
            },
        );

        Ok(stat)
    }

    async fn get_digests_ttl(
        &self,
        digests: Vec<TDigest>,
        metadata: &RemoteExecutionMetadata,
        is_for_upload: bool,
    ) -> yak_error::Result<GetDigestsTtlResponse> {
        with_error_handler(
            "get_digests_ttl",
            self.get_session_id(),
            self.client()
                .get_cas_client()
                .get_digests_ttl(
                    metadata,
                    GetDigestsTtlRequest {
                        digests,
                        is_for_upload: Some(is_for_upload),
                        ..Default::default()
                    },
                )
                .await,
        )
    }

    async fn extend_digest_ttl(
        &self,
        digests: Vec<TDigest>,
        ttl: Duration,
        metadata: &RemoteExecutionMetadata,
    ) -> yak_error::Result<()> {
        // TODO(arr): use batch API from RE when it becomes available
        with_error_handler(
            "extend_digest_ttl",
            self.get_session_id(),
            self.client()
                .get_cas_client()
                .extend_digest_ttl(
                    metadata,
                    ExtendDigestsTtlRequest {
                        digests,
                        ttl: ttl.as_secs() as i64,
                        ..Default::default()
                    },
                )
                .await,
        )?;
        Ok(())
    }

    async fn write_action_result(
        &self,
        digest: ActionDigest,
        result: &mut TActionResult2,
        use_case: RemoteExecutorUseCase,
        platform: &RE::Platform,
        write_type: ActionCacheWriteType,
    ) -> yak_error::Result<WriteActionResultResponse> {
        let write = {
            // The request needs to own the result; lend it for serialization and hand it back.
            let request = WriteActionResultRequest {
                action_digest: digest.to_re(),
                action_result: std::mem::take(result),
                platform: Some(platform.clone()),
                ..Default::default()
            };
            let attributes =
                BTreeMap::from([("write_type".to_owned(), write_type.as_str().to_owned())]);
            let metadata = RemoteExecutionMetadata {
                platform: Some(platform.clone()),
                client_context: Some(TClientContextMetadata {
                    attributes,
                    ..Default::default()
                }),
                ..use_case.metadata(None)
            };
            let write = self
                .client()
                .get_action_cache_client()
                .write_action_result(&metadata, &request);
            *result = request.action_result;
            write
        };
        let response =
            with_error_handler("write_action_result", self.get_session_id(), write.await)?;

        Ok(response)
    }
}

/// Drop the REClient on a blocking thread. The REClient destructor does a blocking wait on async
/// calls (it tells the server to cancel its calls, but it waits for an ack), so we shouldn't drop
/// it on a runtime thread.
impl Drop for RemoteExecutionClientImpl {
    fn drop(&mut self) {
        tracing::info!("Dropping RE client");
        let client = self.client.take();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(move || {
                drop(client);
                tracing::info!("Dropped RE client");
            });
        }
    }
}

fn chunks<T>(v: Vec<T>, chunk_size: usize) -> impl ExactSizeIterator<Item = Vec<T>> {
    if !v.is_empty() && v.len() <= chunk_size {
        return Either::Left(std::iter::once(v));
    }

    let chunks = v
        .into_iter()
        .chunks(chunk_size)
        .into_iter()
        .map(|c| c.collect::<Vec<_>>())
        .collect::<Vec<_>>();

    Either::Right(chunks.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunks_skips() {
        assert_eq!(chunks(Vec::<usize>::new(), 1).next(), None);
    }

    #[test]
    fn test_chunks_reuses() {
        let v = vec![1, 2, 3];
        let addr = v.as_ptr() as usize;
        let mut it = chunks(v, 3);
        let first = it.next().unwrap();
        assert_eq!(first.as_ptr() as usize, addr);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_chunks_splits() {
        let v = vec![1, 2, 3];
        let mut it = chunks(v, 2);
        assert_eq!(it.next(), Some(vec![1, 2]));
        assert_eq!(it.next(), Some(vec![3]));
        assert_eq!(it.next(), None);
    }
}
