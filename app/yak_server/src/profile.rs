/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use dice::DiceTransaction;
use dice_futures::spawn::spawn_dropcancel;
use dupe::Dupe;
use futures::future::FutureExt;
use yak_analysis::analysis::calculation::profile_analysis;
use yak_cli_proto::TargetCfg;
use yak_cli_proto::profile_request::ProfileOpts;
use yak_cli_proto::target_profile::Action;
use yak_common::pattern::parse_from_cli::parse_and_resolve_patterns_from_cli_args;
use yak_core::package::PackageLabel;
use yak_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use yak_core::pattern::pattern_type::TargetPatternExtra;
use yak_core::target::configured_target_label::ConfiguredTargetLabel;
use yak_error::BuckErrorContext;
use yak_error::BuckErrorOptionContext;
use yak_error::internal_error;
use yak_fs::paths::abs_path::AbsPath;
use yak_interpreter::dice::starlark_provider::StarlarkEvalKind;
use yak_interpreter::starlark_profiler::config::GetStarlarkProfilerInstrumentation;
use yak_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use yak_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use yak_interpreter::starlark_profiler::mode::StarlarkProfileMode;
use yak_node::nodes::frontend::TargetGraphCalculation;
use yak_profile::get_profile_response;
use yak_profile::starlark_profiler_configuration_from_request;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::partial_result_dispatcher::NoPartialResult;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use yak_server_ctx::pattern_parse_and_resolve::parse_and_resolve_patterns_to_targets_from_cli_args;
use yak_server_ctx::target_resolution_config::TargetResolutionConfig;
use yak_server_ctx::template::ServerCommandTemplate;
use yak_server_ctx::template::run_server_command;

async fn generate_profile_analysis(
    ctx: DiceTransaction,
    server_ctx: &dyn ServerCommandContextTrait,
    target_patterns: &[String],
    target_resolution_config: TargetResolutionConfig,
    profile_mode: &StarlarkProfilerConfiguration,
) -> yak_error::Result<Arc<StarlarkProfileDataAndStats>> {
    let targets = parse_and_resolve_patterns_to_targets_from_cli_args::<
        ConfiguredProvidersPatternExtra,
    >(&mut ctx.ctx(), target_patterns, server_ctx.working_dir())
    .await?;

    let target_resolution_config = &target_resolution_config;
    let configured_targetss = ctx
        .ctx()
        .try_compute_join(targets, async |ctx, label| {
            target_resolution_config
                .get_configured_target(ctx, &label.target_label, None)
                .await
        })
        .await?;

    let configured_targets: Vec<ConfiguredTargetLabel> =
        configured_targetss.into_iter().flatten().collect();

    match profile_mode {
        StarlarkProfilerConfiguration::ProfileAnalysis(..) => {
            profile_analysis(&mut ctx.ctx(), &configured_targets)
                .await
                .buck_error_context("Recursive profile analysis failed")
                .map(Arc::new)
        }
        _ => Err(internal_error!("Incorrect profile mode")),
    }
}

async fn generate_profile_loading(
    ctx: &DiceTransaction,
    package: PackageLabel,
) -> yak_error::Result<StarlarkProfileDataAndStats> {
    // Self-check.
    let profile_mode = ctx
        .ctx()
        .get_starlark_profiler_mode(&StarlarkEvalKind::LoadBuildFile(package.dupe()))
        .await?;
    match profile_mode {
        StarlarkProfileMode::None => {
            return Err(internal_error!("profile mode must be set in DICE"));
        }
        StarlarkProfileMode::Profile(_) => {}
    }

    let eval_result = ctx.ctx().get_interpreter_results(package).await?;

    let starlark_profile = &eval_result
        .starlark_profile
        .as_ref()
        .internal_error("profile result must be set")?;
    Ok(StarlarkProfileDataAndStats::downcast(&***starlark_profile)?.clone())
}

pub async fn profile_command(
    ctx: &dyn ServerCommandContextTrait,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: yak_cli_proto::ProfileRequest,
) -> yak_error::Result<yak_cli_proto::ProfileResponse> {
    run_server_command(ProfileServerCommand { req }, ctx, partial_result_dispatcher).await
}

struct ProfileServerCommand {
    req: yak_cli_proto::ProfileRequest,
}

#[async_trait]
impl ServerCommandTemplate for ProfileServerCommand {
    type StartEvent = yak_data::ProfileCommandStart;
    type EndEvent = yak_data::ProfileCommandEnd;
    type Response = yak_cli_proto::ProfileResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        ctx: DiceTransaction,
    ) -> yak_error::Result<Self::Response> {
        let output = AbsPath::new(Path::new(&self.req.destination_path))?;

        let profile_mode =
            starlark_profiler_configuration_from_request(&self.req, server_ctx.project_root())?;

        match self
            .req
            .profile_opts
            .as_ref()
            .expect("Target profile not populated")
        {
            ProfileOpts::TargetProfile(opts) => {
                let action = yak_cli_proto::target_profile::Action::try_from(opts.action)
                    .buck_error_context("Invalid action")?;

                let profile_data = generate_profile(
                    server_ctx,
                    ctx,
                    &opts.target_patterns,
                    opts.target_cfg
                        .as_ref()
                        .internal_error("target_cfg not set")?,
                    &opts.target_universe,
                    action,
                    &profile_mode,
                )
                .await?;

                Ok(get_profile_response(
                    profile_data,
                    &opts.target_patterns,
                    output,
                )?)
            }
            _ => {
                return Err(yak_error::yak_error!(
                    yak_error::ErrorTag::Input,
                    "{}",
                    "Expected target profile opts, not BXL profile opts"
                ));
            }
        }
    }
}

async fn generate_profile(
    server_ctx: &dyn ServerCommandContextTrait,
    ctx: DiceTransaction,
    target_patterns: &[String],
    target_cfg: &TargetCfg,
    target_universe: &[String],
    action: Action,
    profile_mode: &StarlarkProfilerConfiguration,
) -> yak_error::Result<Arc<StarlarkProfileDataAndStats>> {
    let target_resolution_config =
        TargetResolutionConfig::from_args(&mut ctx.ctx(), target_cfg, server_ctx, target_universe)
            .await?;

    match action {
        Action::Analysis => {
            generate_profile_analysis(
                ctx,
                server_ctx,
                target_patterns,
                target_resolution_config,
                profile_mode,
            )
            .await
        }
        Action::Loading => {
            let resolved = parse_and_resolve_patterns_from_cli_args::<TargetPatternExtra>(
                &mut ctx.ctx(),
                target_patterns,
                server_ctx.working_dir(),
            )
            .await?;

            let ctx = &ctx;
            let ctx_data = ctx.per_transaction_data();

            let profiles = yak_util::future::try_join_all(resolved.specs.into_iter().map(
                |(package_with_modifiers, _spec)| {
                    let ctx = ctx.dupe();
                    spawn_dropcancel(
                        move |_cancel| {
                            async move {
                                generate_profile_loading(&ctx, package_with_modifiers.package).await
                            }
                            .boxed()
                        },
                        &*ctx_data.spawner,
                        ctx_data,
                    )
                },
            ))
            .await?;

            Ok(StarlarkProfileDataAndStats::merge(profiles.iter()).map(Arc::new)?)
        }
    }
}
