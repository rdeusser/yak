/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::common::YakArgMatches;
use yak_client_ctx::daemon::client::connect::YakdProcessInfo;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::thread_dump::thread_dump_command;
use yak_error::YakErrorContext;
use yak_error::ErrorTag;
use yak_error::yak_error;

/// Prints a thread dump of the currently running yak daemon to stdout
#[derive(Debug, clap::Parser)]
pub struct ThreadDumpCommand {}

impl ThreadDumpCommand {
    pub fn exec(self, _matches: YakArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let paths = ctx.paths()?;
        let daemon_dir = paths.daemon_dir()?;
        let Ok(info) = YakdProcessInfo::load(&daemon_dir) else {
            return yak_error!(ErrorTag::Input, "No running yak daemon").into();
        };

        ctx.with_runtime(|_| async move {
            let status = thread_dump_command(&info)?
                .spawn()
                .yak_error_context("Could not run LLDB to grab a thread-dump")?
                .wait()
                .await?;
            if status.success() {
                yak_error::Ok(ExitResult::success())
            } else {
                // We don't capture stderr, so lldb should have printed an error
                yak_error::Ok(ExitResult::err(yak_error!(
                    ErrorTag::Tier0,
                    "Thread dump command failed"
                )))
            }
        })?
    }
}
