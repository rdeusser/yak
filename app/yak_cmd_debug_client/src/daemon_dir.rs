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
use yak_client_ctx::exit_result::ExitResult;

/// Print yak daemon directory (`~/.yak/yakd/xxx`).
#[derive(Debug, clap::Parser)]
pub struct DaemonDirCommand {}

impl DaemonDirCommand {
    pub fn exec(self, _matches: YakArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        yak_client_ctx::println!("{}", ctx.paths()?.daemon_dir()?.path.display())?;
        ExitResult::success()
    }
}
