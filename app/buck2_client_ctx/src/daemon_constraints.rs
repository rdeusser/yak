/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use buck2_common::init::DaemonStartupConfig;
use buck2_events::daemon_id::DaemonId;

use crate::version::BuckVersion;

/// Checks an environment variable to see if we were spawned by a buck daemon and if so, returns the
/// UUID of that daemon.
///
/// This is used to detect nested invocations, but returning `Some` does not guarantee that this is
/// a nested invocation.
pub fn get_possibly_nested_invocation_daemon_uuid() -> Option<String> {
    // Intentionally don't use `buck2_env!` because we don't want this showing up in help output
    std::env::var("YAK_DAEMON_UUID").ok()
}

/// Generates the daemon constraints *for the currently running daemon.*
///
/// Note that this function is called *from the daemon* and represents the daemon's constraints -
/// the constraints that the client would like the daemon to have are generated separately.
pub fn gen_daemon_constraints(
    daemon_startup_config: &DaemonStartupConfig,
    daemon_id: &DaemonId,
) -> buck2_error::Result<buck2_cli_proto::DaemonConstraints> {
    Ok(buck2_cli_proto::DaemonConstraints {
        version: version()?,
        daemon_id: daemon_id.to_string(),
        daemon_startup_config: Some(daemon_startup_config.serialize()?),
        extra: None,
    })
}

pub fn version() -> buck2_error::Result<String> {
    Ok(BuckVersion::get_unique_id()?.to_owned())
}
