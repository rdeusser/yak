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

use dice::DiceComputations;
use futures::FutureExt;
use yak_core::package::PackageLabel;
use yak_error::YakErrorContext;
use yak_error::YakErrorOptionContext;
use yak_events::dispatch::console_message;
use yak_fs::paths::abs_path::AbsPath;
use yak_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use yak_node::nodes::frontend::TargetGraphCalculation;
use yak_profile::write_starlark_profile;

pub(crate) async fn write_query_profile_for_targets(
    ctx: &mut DiceComputations<'_>,
    _profile_mode: yak_cli_proto::ProfileMode,
    output_path: Option<&str>,
    targets: impl IntoIterator<Item = PackageLabel>,
) -> yak_error::Result<()> {
    let output_path = output_path.internal_error("Outut path must be set for profile mode")?;
    let output_path = AbsPath::new(Path::new(output_path))
        .yak_error_context("Output path must be set to absolute path by the client")?;
    do_write_query_profile_for_targets(ctx, output_path, Vec::from_iter(targets))
        .boxed()
        .await
}

async fn do_write_query_profile_for_targets(
    ctx: &mut DiceComputations<'_>,
    output_path: &AbsPath,
    mut targets: Vec<PackageLabel>,
) -> yak_error::Result<()> {
    // We want stable output.
    targets.sort();
    targets.dedup();

    let mut profiles = Vec::new();
    let mut target_names = Vec::new();
    for target in targets {
        // This should be already cached.
        target_names.push(target.to_string());
        let eval_results = ctx.get_interpreter_results(target).await?;
        let profile = eval_results
            .starlark_profile
            .as_ref()
            .internal_error("Starlark profile must be set")?;
        let profile = StarlarkProfileDataAndStats::downcast(&**profile)?;
        profiles.push(profile.clone());
    }
    let profile = StarlarkProfileDataAndStats::merge(&profiles)?;

    write_starlark_profile(&profile, &target_names, output_path)?;

    console_message(format!(
        "Starlark profile data is written to {}",
        output_path.display()
    ));

    Ok(())
}
