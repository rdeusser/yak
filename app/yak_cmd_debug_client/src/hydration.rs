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
use yak_cli_proto::HydrationRequest;
use yak_cli_proto::HydrationSubcommand;
use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::common::CommonBuildConfigurationOptions;
use yak_client_ctx::common::CommonEventLogOptions;
use yak_client_ctx::common::CommonStarlarkOptions;
use yak_client_ctx::common::YakArgMatches;
use yak_client_ctx::common::ui::CommonConsoleOptions;
use yak_client_ctx::daemon::client::NoPartialResultHandler;
use yak_client_ctx::daemon::client::YakdClientConnector;
use yak_client_ctx::events_ctx::EventsCtx;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::streaming::StreamingCommand;
use yak_error::YakErrorContext;

/// Subcommands for `yak debug hydration`.
#[derive(Debug, clap::Parser)]
pub enum HydrationCommand {
    /// Page out DICE values to storage.
    PageOut(PageOutCommand),
    /// Page in DICE values from storage.
    PageIn(PageInCommand),
    /// Summarize which DICE node values are resident in memory vs paged out.
    Status(StatusCommand),
}

#[derive(Debug, clap::Parser)]
pub struct PageOutCommand {
    #[clap(flatten)]
    event_log_opts: CommonEventLogOptions,
}

#[derive(Debug, clap::Parser)]
pub struct PageInCommand {
    #[clap(flatten)]
    event_log_opts: CommonEventLogOptions,
}

#[derive(Debug, clap::Parser)]
pub struct StatusCommand {
    /// Block until any in-progress idle page-out finishes before reporting.
    #[clap(long)]
    wait: bool,

    #[clap(flatten)]
    event_log_opts: CommonEventLogOptions,
}

impl HydrationCommand {
    fn subcommand(&self) -> HydrationSubcommand {
        match self {
            HydrationCommand::PageOut(_) => HydrationSubcommand::PageOut,
            HydrationCommand::PageIn(_) => HydrationSubcommand::PageIn,
            HydrationCommand::Status(_) => HydrationSubcommand::Status,
        }
    }

    fn wait(&self) -> bool {
        match self {
            HydrationCommand::Status(c) => c.wait,
            HydrationCommand::PageOut(_) | HydrationCommand::PageIn(_) => false,
        }
    }
}

#[async_trait(?Send)]
impl StreamingCommand for HydrationCommand {
    const COMMAND_NAME: &'static str = "hydration";

    fn existing_only() -> bool {
        true
    }

    async fn exec_impl(
        self,
        yakd: &mut YakdClientConnector,
        _matches: YakArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let context = ctx.empty_client_context("debug-hydration")?;
        let response = yakd
            .with_flushing()
            .hydration(
                HydrationRequest {
                    context: Some(context),
                    subcommand: self.subcommand().into(),
                    wait: self.wait(),
                },
                events_ctx,
                ctx.console_interaction_stream(self.console_opts()),
                &mut NoPartialResultHandler,
            )
            .await??;

        // Only `status` returns a report; page-out / page-in leave it `None`.
        if let Some(summary) = response.summary {
            yak_client_ctx::println!("{}", summary.trim_end())?;
        }
        // JSON consumed by tests and tooling, fields match the `yak_page_outs` columns.
        if let Some(page_out_summary) = response.page_out_summary {
            let json = serde_json::to_string_pretty(&page_out_summary)
                .yak_error_context("Failed to serialize page-out summary")?;
            yak_client_ctx::println!("{}", json)?;
        }
        ExitResult::success()
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        CommonConsoleOptions::default_ref()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        match self {
            HydrationCommand::PageOut(c) => &c.event_log_opts,
            HydrationCommand::PageIn(c) => &c.event_log_opts,
            HydrationCommand::Status(c) => &c.event_log_opts,
        }
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        CommonBuildConfigurationOptions::default_ref()
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        CommonStarlarkOptions::default_ref()
    }
}
