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
use jiff::SignedDuration;
use jiff::Timestamp;
use yak_cli_proto::CleanStaleRequest;
use yak_cli_proto::CleanStaleResponse;
use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::common::CommonBuildConfigurationOptions;
use yak_client_ctx::common::CommonCommandOptions;
use yak_client_ctx::common::CommonEventLogOptions;
use yak_client_ctx::common::CommonStarlarkOptions;
use yak_client_ctx::common::YakArgMatches;
use yak_client_ctx::common::ui::CommonConsoleOptions;
use yak_client_ctx::daemon::client::NoPartialResultHandler;
use yak_client_ctx::daemon::client::YakdClientConnector;
use yak_client_ctx::daemon::client::connect::DaemonStartupMode;
use yak_client_ctx::events_ctx::EventsCtx;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::streaming::StreamingCommand;
use yak_error::YakErrorContext;
use yak_error::conversion::from_any_with_tag;
use yak_error::internal_error;

/// Clean only old artifacts from a running yak daemon without killing the daemon.
/// This can be interrupted by other commands that run in parallel and request materialization.
///
/// This is a separate command from CleanCommand even though it is invoked with
/// a flag (--stale) on the clean subcommand, which is a bit weird.
/// This is just so that it can be used as a StreamingCommand, which CleanCommand should not be.
pub struct CleanStaleCommand {
    pub(crate) common_opts: CommonCommandOptions,
    pub keep_since_arg: KeepSinceArg,
    pub dry_run: bool,
    pub tracked_only: bool,
    /// Free-disk % threshold for adaptive low-disk promotion. None disables it.
    pub adaptive_low_disk_threshold: Option<f64>,
    /// Adaptive min-TTL floor. None means use the daemon-side default (12h).
    pub adaptive_min_ttl: Option<std::time::Duration>,
    pub adaptive_unmaterialize_active: bool,
}

/// Specifies the maximum age of artifacts to keep
pub enum KeepSinceArg {
    Configured,
    Duration(SignedDuration),
    Time(i64),
}

pub fn parse_clean_stale_args(
    stale: Option<Option<humantime::Duration>>,
    keep_since_time: Option<i64>,
) -> yak_error::Result<Option<KeepSinceArg>> {
    let arg = match (stale, keep_since_time) {
        (Some(Some(human_duration)), None) => {
            let duration = SignedDuration::try_from(std::time::Duration::from(human_duration))
                .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::InvalidDuration))?;
            Some(KeepSinceArg::Duration(duration))
        }
        (Some(None), None) => Some(KeepSinceArg::Configured),
        (None, Some(time)) => Some(KeepSinceArg::Time(time)),
        (Some(_), Some(_)) => unreachable!("keep-since-time conflicts_with stale"),
        (None, None) => None,
    };
    Ok(arg)
}

fn format_result_stats(stats: yak_data::CleanStaleStats) -> String {
    let mut output = String::new();
    output += &format!(
        "Found {} stale artifacts ({})\n",
        stats.stale_artifact_count,
        bytesize::ByteSize::b(stats.stale_bytes).display().iec(),
    );
    output += &format!(
        "Found {} recent artifacts ({})\n",
        stats.retained_artifact_count,
        bytesize::ByteSize::b(stats.retained_bytes).display().iec(),
    );
    output += &format!(
        "Found {} untracked artifacts ({})\n",
        stats.untracked_artifact_count,
        bytesize::ByteSize::b(stats.untracked_bytes).display().iec(),
    );
    if stats.skipped_unreadable_count > 0 {
        output += &format!(
            "Skipped {} paths that could not be read or deleted (permission denied)\n",
            stats.skipped_unreadable_count,
        );
    }
    if stats.cleaned_artifact_count > 0 || stats.cleaned_bytes > 0 {
        output += &format!("Cleaned {} paths\n", stats.cleaned_artifact_count,);
        output += &format!(
            "{} bytes cleaned ({})\n",
            stats.cleaned_bytes,
            bytesize::ByteSize::b(stats.cleaned_bytes).display().iec(),
        );
    }
    if stats.unmaterialized_only_artifact_count > 0 || stats.unmaterialized_only_bytes > 0 {
        output += &format!(
            "Unmaterialized {} artifacts ({})\n",
            stats.unmaterialized_only_artifact_count,
            bytesize::ByteSize::b(stats.unmaterialized_only_bytes)
                .display()
                .iec(),
        );
    }
    output
}

#[async_trait(?Send)]
impl StreamingCommand for CleanStaleCommand {
    const COMMAND_NAME: &'static str = "clean-stale";

    fn daemon_startup_mode() -> DaemonStartupMode {
        DaemonStartupMode::CleanStale
    }

    async fn exec_impl(
        self,
        yakd: &mut YakdClientConnector,
        matches: YakArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let (keep_since_time, use_configured_policy) = match self.keep_since_arg {
            KeepSinceArg::Configured => {
                yak_client_ctx::eprintln!(
                    "Cleaning artifacts using the configured clean-stale policy"
                )?;
                (Timestamp::UNIX_EPOCH, true)
            }
            KeepSinceArg::Duration(duration) => {
                let keep_since_time = Timestamp::now()
                    .checked_sub(duration)
                    .map_err(|_| internal_error!("Duration underflow"))?;
                yak_client_ctx::eprintln!(
                    "Cleaning artifacts more than {} old",
                    humantime::format_duration(
                        std::time::Duration::try_from(duration)
                            .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::InvalidDuration))
                            .yak_error_context("Error converting duration")?
                    ),
                )?;
                // Round up to next second since timestamp below is rounded down
                // (this way clean --stale=0s immediately after a build deletes the result)
                let keep_since_time = keep_since_time
                    .checked_add(SignedDuration::from_secs(1))
                    .map_err(|_| internal_error!("Timestamp overflow"))?;
                (keep_since_time, false)
            }
            KeepSinceArg::Time(timestamp) => (
                Timestamp::from_second(timestamp)
                    .map_err(|_| internal_error!("Invalid timestamp"))?,
                false,
            ),
        };

        if let Some(threshold) = self.adaptive_low_disk_threshold {
            let min_ttl_msg = match self.adaptive_min_ttl {
                Some(d) => humantime::format_duration(d).to_string(),
                None => "daemon default".to_owned(),
            };
            yak_client_ctx::eprintln!(
                "Adaptive low-disk promotion enabled at {}% (min TTL: {})",
                threshold,
                min_ttl_msg,
            )?;
        }

        let context = ctx.client_context(matches, &self)?;
        let response: CleanStaleResponse = yakd
            .with_flushing()
            .clean_stale(
                CleanStaleRequest {
                    context: Some(context),
                    keep_since_time: keep_since_time.as_second(),
                    dry_run: self.dry_run,
                    tracked_only: self.tracked_only,
                    adaptive_low_disk_threshold: self.adaptive_low_disk_threshold,
                    adaptive_min_ttl_seconds: self.adaptive_min_ttl.map(|d| d.as_secs() as i64),
                    adaptive_unmaterialize_active: self.adaptive_unmaterialize_active,
                    use_configured_policy,
                },
                events_ctx,
                ctx.console_interaction_stream(&self.common_opts.console_opts),
                &mut NoPartialResultHandler,
            )
            .await??;

        if let Some(message) = response.message {
            yak_client_ctx::eprintln!("{}", message)?;
        }
        if let Some(stats) = response.stats {
            yak_client_ctx::eprintln!("{}", format_result_stats(stats))?;
        }
        ExitResult::success()
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
