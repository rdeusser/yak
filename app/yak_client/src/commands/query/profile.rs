/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_client_ctx::common::profiling::YakProfileMode;
use yak_client_ctx::path_arg::PathArg;

/// Starlark profiling options
#[derive(Debug, Clone, clap::Parser)]
#[clap(next_help_heading = "Starlark Profiling Options")]
pub(crate) struct QueryProfileOptions {
    /// Profile target loading.
    ///
    /// When this option is enabled, yak will profile every `YAK` file loaded during the query
    /// and merge the results into a single profile.
    /// The command may return cached profile data if `YAK` files were not invalidated.
    #[clap(long, requires("profile_output"))]
    pub(crate) profile_mode: Option<YakProfileMode>,

    /// Where to write profile output.
    #[clap(long)]
    pub(crate) profile_output: Option<PathArg>,
}

impl QueryProfileOptions {
    pub(crate) fn profile_mode_proto(&self) -> Option<yak_cli_proto::ProfileMode> {
        self.profile_mode.map(|v| v.to_proto())
    }
}
