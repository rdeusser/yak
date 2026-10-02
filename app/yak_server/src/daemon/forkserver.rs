/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_common::legacy_configs::configs::LegacyYakConfig;
use yak_execute_impl::executors::local::ForkserverAccess;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_fs::paths::file_name::FileNameBuf;
use yak_resource_control::yak_cgroup_tree::YakCgroupTree;

#[cfg(unix)]
pub async fn maybe_launch_forkserver(
    root_config: &LegacyYakConfig,
    forkserver_state_dir: &AbsNormPath,
    cgroup_tree: Option<&YakCgroupTree>,
    isolation_dir: &FileNameBuf,
) -> yak_error::Result<ForkserverAccess> {
    use yak_common::legacy_configs::key::YakconfigKeyRef;
    use yak_core::rollout_percentage::RolloutPercentage;
    use yak_error::YakErrorContext;

    let config = root_config
        .parse::<RolloutPercentage>(YakconfigKeyRef {
            section: "yak",
            property: "forkserver",
        })?
        .unwrap_or_else(RolloutPercentage::always);

    if !config.roll() {
        return Ok(ForkserverAccess::None);
    }

    let exe = std::env::current_exe().yak_error_context("Cannot access current_exe")?;
    Ok(ForkserverAccess::Client(
        yak_forkserver::launch::launch_forkserver(
            exe,
            // `--isolation-dir` is not read by the forkserver itself; it is carried on the
            // command line so that `yak killall --in-isolation-dir` can attribute the process.
            &["forkserver", "--isolation-dir", isolation_dir.as_str()],
            forkserver_state_dir,
            cgroup_tree,
        )
        .await?,
    ))
}

#[cfg(not(unix))]
pub async fn maybe_launch_forkserver(
    _root_config: &LegacyYakConfig,
    _forkserver_state_dir: &AbsNormPath,
    _cgroup_tree: Option<&YakCgroupTree>,
    _isolation_dir: &FileNameBuf,
) -> yak_error::Result<ForkserverAccess> {
    Ok(ForkserverAccess::None)
}
