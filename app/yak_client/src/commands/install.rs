/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use yak_cli_proto::InstallRequest;
use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::command_outcome::CommandOutcome;
use yak_client_ctx::common::BuckArgMatches;
use yak_client_ctx::common::CommonBuildConfigurationOptions;
use yak_client_ctx::common::CommonCommandOptions;
use yak_client_ctx::common::CommonEventLogOptions;
use yak_client_ctx::common::CommonStarlarkOptions;
use yak_client_ctx::common::build::CommonBuildOptions;
use yak_client_ctx::common::target_cfg::TargetCfgOptions;
use yak_client_ctx::common::ui::CommonConsoleOptions;
use yak_client_ctx::daemon::client::BuckdClientConnector;
use yak_client_ctx::daemon::client::NoPartialResultHandler;
use yak_client_ctx::events_ctx::EventsCtx;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::streaming::StreamingCommand;

use crate::commands::build::print_build_id;

#[derive(Debug, clap::Parser)]
#[clap(name = "install", about = "Build and install an application")]
pub struct InstallCommand {
    #[clap(
        long,
        name = "installer-debug",
        help = "Prints installer output to stderr. It might break superconsole"
    )]
    installer_debug: bool,

    #[clap(name = "TARGET", help = "Target to build and install", value_hint = clap::ValueHint::Other)]
    patterns: Vec<String>,

    #[clap(
        name = "INSTALL_ARGS",
        help = "Additional arguments passed to the install when running it",
        raw = true
    )]
    extra_run_args: Vec<String>,

    #[clap(flatten)]
    build_opts: CommonBuildOptions,

    #[clap(flatten)]
    target_cfg: TargetCfgOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

#[async_trait(?Send)]
impl StreamingCommand for InstallCommand {
    const COMMAND_NAME: &'static str = "install";
    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        matches: BuckArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let context = ctx.client_context(matches, &self)?;

        let response = buckd
            .with_flushing()
            .install(
                InstallRequest {
                    context: Some(context),
                    target_patterns: self.patterns.clone(),
                    target_cfg: Some(self.target_cfg.target_cfg()),
                    build_opts: Some(self.build_opts.to_proto()),
                    installer_run_args: self.extra_run_args.clone(),
                    installer_debug: self.installer_debug,
                },
                events_ctx,
                ctx.console_interaction_stream(&self.common_opts.console_opts),
                &mut NoPartialResultHandler,
            )
            .await?;
        let console = self.common_opts.console_opts.final_console();
        print_build_id(&console, ctx, events_ctx.used_superconsole)?;

        match response {
            CommandOutcome::Success(_) => {
                if self.patterns.is_empty() {
                    console.print_warning("NO BUILD TARGET PATTERNS SPECIFIED")?;
                } else if ctx.verbosity.print_success_message() {
                    console.print_success("INSTALL SUCCEEDED")?;
                }
                ExitResult::success()
            }
            CommandOutcome::Failure(exit_result) => {
                console.print_error("INSTALL FAILED")?;
                exit_result
            }
        }
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        &self.common_opts.console_opts
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.common_opts.event_log_opts
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        &self.common_opts.config_opts
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        &self.common_opts.starlark_opts
    }
}
