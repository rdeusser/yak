/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Targets that build only for an execution platform.

use dice::DiceComputations;
use yak_common::dice::cells::HasCellResolver;
use yak_common::legacy_configs::dice::HasLegacyConfigs;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_core::configuration::compatibility::IncompatiblePlatformReason;
use yak_core::configuration::compatibility::IncompatiblePlatformReasonCause;

/// The constraint value that the prelude adds to every execution platform when
/// `[build] exec_platform_marker` is unset (`prelude/cfg/exec_platform/marker.bzl`).
const DEFAULT_EXEC_PLATFORM_MARKER: &str =
    "prelude//cfg/exec_platform/marker:is_exec_platform[true]";

/// Reports whether `reason` makes a target incompatible only because its `target_compatible_with`
/// requires the execution platform marker. Such a target, like the build script of a Cargo
/// workspace member, builds as the exec dependency of another target, so a pattern that matches
/// it skips it without listing it.
pub async fn is_exec_only(
    ctx: &mut DiceComputations<'_>,
    reason: &IncompatiblePlatformReason,
) -> yak_error::Result<bool> {
    let IncompatiblePlatformReasonCause::UnsatisfiedConfig(unsatisfied) = &reason.cause else {
        return Ok(false);
    };
    let root_cell = ctx.get_cell_resolver().await?.root_cell();
    let marker = ctx
        .get_legacy_config_property(
            root_cell,
            YakconfigKeyRef {
                section: "build",
                property: "exec_platform_marker",
            },
        )
        .await?;
    let marker = marker.as_deref().unwrap_or(DEFAULT_EXEC_PLATFORM_MARKER);
    Ok(unsatisfied.to_string() == marker)
}
