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
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::DiceTransaction;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::future::BoxFuture;
use yak_build_signals::env::BuildSignalsContext;
use yak_build_signals::env::DeferredBuildSignals;
use yak_build_signals::env::HasCriticalPathBackend;
use yak_cli_proto::client_context::ExitWhen;
use yak_cli_proto::client_context::PreemptibleWhen;
use yak_common::legacy_configs::dice::HasInjectedLegacyConfigs;
use yak_core::fs::project::ProjectRoot;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::pattern::pattern::ParsedPattern;
use yak_core::pattern::pattern::ParsedPatternWithModifiers;
use yak_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use yak_data::CommandCriticalEnd;
use yak_data::CommandCriticalStart;
use yak_data::DiceCriticalSectionEnd;
use yak_data::DiceCriticalSectionStart;
use yak_events::dispatch::EventDispatcher;
use yak_execute::materialize::materializer::Materializer;
use yak_fs::paths::file_name::FileName;
use yak_fs::working_dir::AbsWorkingDir;
use yak_hash::IntentionallyStdHashMap;
use yak_util::early_command_timing::EarlyCommandTimingBuilder;
use yak_wrapper_common::invocation_id::TraceId;

use crate::concurrency::CommandEvents;
use crate::concurrency::CommandTransactionObserver;
use crate::concurrency::ConcurrencyHandler;
use crate::concurrency::DiceUpdater;
use crate::stderr_output_guard::StderrOutputGuard;

#[derive(Allocative, Debug)]
pub struct PreviousCommandDataInternal {
    pub external_and_local_configs: Vec<yak_data::YakconfigComponent>,
    pub sanitized_argv: Vec<String>,
    pub trace_id: TraceId,
}

#[derive(Allocative, Debug, Default)]
pub struct PreviousCommandData {
    pub data: Option<PreviousCommandDataInternal>,
}

impl PreviousCommandData {
    pub fn process_current_command(
        &mut self,
        event_dispatcher: EventDispatcher,
        current_external_and_local_configs: Vec<yak_data::YakconfigComponent>,
        current_sanitized_argv: Vec<String>,
        current_trace: TraceId,
    ) {
        if let Some(PreviousCommandDataInternal {
            external_and_local_configs: external_configs,
            sanitized_argv,
            trace_id,
        }) = self.data.as_ref()
        {
            if *current_external_and_local_configs != *external_configs {
                event_dispatcher.instant_event(yak_data::PreviousCommandWithMismatchedConfig {
                    sanitized_argv: sanitized_argv.clone(),
                    trace_id: trace_id.to_string(),
                });
            }
        }

        self.data = Some(PreviousCommandDataInternal {
            external_and_local_configs: current_external_and_local_configs,
            sanitized_argv: current_sanitized_argv,
            trace_id: current_trace,
        });
    }
}

#[derive(Allocative, Debug, Default)]
pub struct LockedPreviousCommandData {
    pub data: Mutex<PreviousCommandData>,
}
impl LockedPreviousCommandData {
    pub fn new() -> Arc<Self> {
        Arc::new(LockedPreviousCommandData {
            data: Mutex::new(PreviousCommandData { data: None }),
        })
    }
}

#[derive(Clone, Dupe)]
pub struct DispatcherEvents(pub EventDispatcher);

impl CommandEvents for DispatcherEvents {
    fn instant(&self, data: yak_data::instant_event::Data) {
        self.0.instant_event(data);
    }

    fn trace_id(&self) -> &TraceId {
        self.0.trace_id()
    }

    fn console_warning(&self, message: String) {
        self.0.console_warning(message);
    }

    fn span<'a, R: Send + 'a>(
        &self,
        start: yak_data::span_start_event::Data,
        fut: BoxFuture<'a, (R, yak_data::span_end_event::Data)>,
    ) -> BoxFuture<'a, R> {
        Box::pin(self.0.span_async(start, fut))
    }
}

/// Emits the yakconfig-derived telemetry for a command: the comparison against the previous
/// command's config and the config values themselves.
/// Concurrent equivalent-state commands update previous-command telemetry in observer completion
/// order, so the stored UUID is a comparison anchor rather than a strict admission predecessor.
struct YakconfigTelemetry<'a> {
    events: EventDispatcher,
    project_root: &'a ProjectRoot,
    previous_command_data: Arc<LockedPreviousCommandData>,
    sanitized_argv: Vec<String>,
    trace_id: TraceId,
}

#[async_trait]
impl CommandTransactionObserver for YakconfigTelemetry<'_> {
    async fn on_transaction_committed(
        &self,
        transaction: &DiceTransaction,
    ) -> yak_error::Result<()> {
        if !transaction
            .ctx()
            .is_injected_external_yakconfig_data_key_set()
            .await?
        {
            return Ok(());
        }

        let external_configs = transaction
            .ctx()
            .get_injected_external_yakconfig_data()
            .await?;
        let current_external_and_local_configs: Vec<yak_data::YakconfigComponent> =
            external_configs
                .get_yakconfig_components(self.project_root)
                .await;

        self.previous_command_data
            .data
            .lock()
            .unwrap()
            .process_current_command(
                self.events.dupe(),
                current_external_and_local_configs.clone(),
                self.sanitized_argv.clone(),
                self.trace_id.dupe(),
            );

        self.events.instant_event(yak_data::YakconfigInputValues {
            components: current_external_and_local_configs,
        });

        Ok(())
    }
}

#[async_trait]
pub trait ServerCommandContextTrait: Send + Sync {
    fn working_dir(&self) -> &ProjectRelativePath;

    fn working_dir_abs(&self) -> &AbsWorkingDir;

    fn command_name(&self) -> &str;

    fn isolation_prefix(&self) -> &FileName;

    fn project_root(&self) -> &ProjectRoot;

    fn materializer(&self) -> Arc<dyn Materializer>;

    /// exposes the dice for scoped access, but isn't intended to be callable by anyone
    async fn dice_accessor<'a>(
        &'a self,
        private: PrivateStruct,
    ) -> yak_error::Result<DiceAccessor<'a>>;

    fn events(&self) -> &EventDispatcher;

    fn previous_command_data(&self) -> Arc<LockedPreviousCommandData>;

    fn stderr(&self) -> yak_error::Result<StderrOutputGuard<'_>>;

    async fn command_start_event(
        &self,
        data: yak_data::command_start::Data,
    ) -> yak_error::Result<yak_data::CommandStart>;

    async fn request_metadata(&self) -> yak_error::Result<IntentionallyStdHashMap<String, String>>;

    async fn config_metadata(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> yak_error::Result<IntentionallyStdHashMap<String, String>>;

    fn log_target_pattern(
        &self,
        providers_patterns: &[ParsedPattern<ConfiguredProvidersPatternExtra>],
    );

    fn log_target_pattern_with_modifiers(
        &self,
        providers_patterns_with_modifiers: &[ParsedPatternWithModifiers<
            ConfiguredProvidersPatternExtra,
        >],
    );

    fn cancellation_context(&self) -> &CancellationContext;

    fn command_start(&self) -> Instant;
}

pub struct PrivateStruct(());

pub struct DiceAccessor<'a> {
    pub dice_handler: Arc<ConcurrencyHandler>,
    pub setup: Box<dyn DiceUpdater + 'a>,
    pub is_nested_invocation: bool,
    pub sanitized_argv: Vec<String>,
    pub preemptible: PreemptibleWhen,
    pub build_signals: Box<dyn DeferredBuildSignals>,
    pub exit_when: ExitWhen,
}

#[async_trait]
pub trait ServerCommandDiceContext {
    async fn with_dice_ctx<'v, F, Fut, R>(&'v self, exec: F) -> yak_error::Result<R>
    where
        F: FnOnce(&'v dyn ServerCommandContextTrait, DiceTransaction) -> Fut + Send,
        Fut: Future<Output = yak_error::Result<R>> + Send,
        R: Send;

    async fn with_dice_ctx_maybe_exclusive<'v, F, Fut, R>(
        &'v self,
        exec: F,
        exclusive_cmd: Option<String>,
    ) -> yak_error::Result<R>
    where
        F: FnOnce(&'v dyn ServerCommandContextTrait, DiceTransaction) -> Fut + Send,
        Fut: Future<Output = yak_error::Result<R>> + Send,
        R: Send;
}

#[async_trait]
impl ServerCommandDiceContext for dyn ServerCommandContextTrait + '_ {
    /// Allows running a section of code that uses the shared DiceTransaction
    async fn with_dice_ctx<'v, F, Fut, R>(&'v self, exec: F) -> yak_error::Result<R>
    where
        F: FnOnce(&'v dyn ServerCommandContextTrait, DiceTransaction) -> Fut + Send,
        Fut: Future<Output = yak_error::Result<R>> + Send,
        R: Send,
    {
        self.with_dice_ctx_maybe_exclusive(exec, None).await
    }

    async fn with_dice_ctx_maybe_exclusive<'v, F, Fut, R>(
        &'v self,
        exec: F,
        exclusive_cmd: Option<String>,
    ) -> yak_error::Result<R>
    where
        F: FnOnce(&'v dyn ServerCommandContextTrait, DiceTransaction) -> Fut + Send,
        Fut: Future<Output = yak_error::Result<R>> + Send,
        R: Send,
    {
        let DiceAccessor {
            dice_handler,
            setup,
            is_nested_invocation,
            sanitized_argv,
            preemptible,
            build_signals,
            exit_when,
        } = self.dice_accessor(PrivateStruct(())).await?;

        let early_command_timing = EarlyCommandTimingBuilder::new(self.command_start());

        let transaction_observer = YakconfigTelemetry {
            events: self.events().dupe(),
            project_root: self.project_root(),
            previous_command_data: self.previous_command_data(),
            sanitized_argv: sanitized_argv.clone(),
            trace_id: self.events().trace_id().dupe(),
        };

        let events = self.events().dupe();
        events
            .span_async(DiceCriticalSectionStart {}, async move {
                (
                    dice_handler
                        .enter(
                            DispatcherEvents(self.events().dupe()),
                            &*setup,
                            |dice, early_command_timing| async move {
                                let events = self.events().dupe();

                                let request_metadata = self.request_metadata().await?;
                                let config_metadata = self.config_metadata(&mut dice.ctx()).await?;
                                events
                                    .span_async(
                                        CommandCriticalStart {
                                            metadata: config_metadata.clone(),
                                            dice_version: dice.equality_token().to_string(),
                                        },
                                        async move {
                                            let res = yak_build_signals::env::scope(
                                                build_signals,
                                                self.events().dupe(),
                                                dice.per_transaction_data()
                                                    .get_critical_path_backend(),
                                                BuildSignalsContext {
                                                    command_name: self.command_name().to_owned(),
                                                    metadata: request_metadata
                                                        .into_iter()
                                                        .chain(
                                                            config_metadata.iter().map(|(k, v)| {
                                                                (k.clone(), v.clone())
                                                            }),
                                                        )
                                                        .collect(),
                                                    isolation_prefix: self
                                                        .isolation_prefix()
                                                        .to_owned(),
                                                    early_command_timing: early_command_timing
                                                        .finish_early_command_timing(),
                                                },
                                                || exec(self, dice),
                                            )
                                            .await;

                                            (
                                                res,
                                                CommandCriticalEnd {
                                                    metadata: config_metadata,
                                                },
                                            )
                                        },
                                    )
                                    .await
                            },
                            is_nested_invocation,
                            sanitized_argv,
                            exclusive_cmd,
                            self.cancellation_context(),
                            preemptible,
                            &transaction_observer,
                            exit_when,
                            early_command_timing,
                        )
                        .await,
                    DiceCriticalSectionEnd {},
                )
            })
            .await?
    }
}
