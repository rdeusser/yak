/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use starlark::eval::ProfileMode;
use yak_cli_proto::HasClientContext;
use yak_cli_proto::profile_request::ProfileOpts;
use yak_core::fs::project::ProjectRoot;
use yak_core::pattern::unparsed::UnparsedPatternPredicate;
use yak_core::pattern::unparsed::UnparsedPatterns;
use yak_error::YakErrorContext;
use yak_error::conversion::from_any_with_tag;
use yak_error::yak_error;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_fs::paths::abs_path::AbsPath;
use yak_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use yak_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;

pub fn proto_to_profile_mode(proto: yak_cli_proto::ProfileMode) -> ProfileMode {
    match proto {
        yak_cli_proto::ProfileMode::HeapAllocated => ProfileMode::HeapAllocated,
        yak_cli_proto::ProfileMode::HeapRetained => ProfileMode::HeapRetained,
        yak_cli_proto::ProfileMode::HeapFlameAllocated => ProfileMode::HeapFlameAllocated,
        yak_cli_proto::ProfileMode::HeapFlameRetained => ProfileMode::HeapFlameRetained,
        yak_cli_proto::ProfileMode::HeapSummaryAllocated => ProfileMode::HeapSummaryAllocated,
        yak_cli_proto::ProfileMode::HeapSummaryRetained => ProfileMode::HeapSummaryRetained,
        yak_cli_proto::ProfileMode::TimeFlame => ProfileMode::TimeFlame,
        yak_cli_proto::ProfileMode::Statement => ProfileMode::Statement,
        yak_cli_proto::ProfileMode::Bytecode => ProfileMode::Bytecode,
        yak_cli_proto::ProfileMode::BytecodePairs => ProfileMode::BytecodePairs,
        yak_cli_proto::ProfileMode::Typecheck => ProfileMode::Typecheck,
        yak_cli_proto::ProfileMode::Coverage => ProfileMode::Coverage,
        yak_cli_proto::ProfileMode::None => ProfileMode::None,
    }
}

pub fn starlark_profiler_configuration_from_request(
    req: &yak_cli_proto::ProfileRequest,
    project_root: &ProjectRoot,
) -> yak_error::Result<StarlarkProfilerConfiguration> {
    let profiler_proto = yak_cli_proto::ProfileMode::try_from(req.profile_mode)
        .yak_error_context("Invalid profiler")?;

    let profile_mode = proto_to_profile_mode(profiler_proto);

    match req.profile_opts.as_ref().expect("Missing profile opts") {
        ProfileOpts::TargetProfile(opts) => {
            let action = yak_cli_proto::target_profile::Action::try_from(opts.action)
                .yak_error_context("Invalid action")?;
            Ok(match (action, opts.recursive) {
                (yak_cli_proto::target_profile::Action::Loading, false) => {
                    let working_dir = AbsNormPath::new(&req.client_context()?.working_dir)?;
                    let working_dir = project_root.relativize(working_dir)?;
                    StarlarkProfilerConfiguration::ProfileLoading(
                        profile_mode,
                        UnparsedPatternPredicate::AnyOf(UnparsedPatterns::new(
                            opts.target_patterns.clone(),
                            working_dir.to_buf(),
                        )),
                    )
                }
                (yak_cli_proto::target_profile::Action::Loading, true) => {
                    return Err(yak_error!(
                        yak_error::ErrorTag::Input,
                        "Recursive profiling is not supported for loading profiling, but you can pass multiple target patterns."
                    ));
                }
                (yak_cli_proto::target_profile::Action::Analysis, false) => {
                    let working_dir = AbsNormPath::new(&req.client_context()?.working_dir)?;
                    let working_dir = project_root.relativize(working_dir)?;
                    StarlarkProfilerConfiguration::ProfileAnalysis(
                        profile_mode,
                        UnparsedPatternPredicate::AnyOf(UnparsedPatterns::new(
                            opts.target_patterns.clone(),
                            working_dir.to_buf(),
                        )),
                    )
                }
                (yak_cli_proto::target_profile::Action::Analysis, true) => {
                    StarlarkProfilerConfiguration::ProfileAnalysis(
                        profile_mode,
                        UnparsedPatternPredicate::Any,
                    )
                }
            })
        }
        ProfileOpts::BxlProfile(_) => Ok(StarlarkProfilerConfiguration::ProfileBxl(profile_mode)),
    }
}

#[allow(clippy::format_collect)]
pub fn write_starlark_profile(
    profile_data: &StarlarkProfileDataAndStats,
    targets: &[String],
    output: &AbsPath,
) -> yak_error::Result<()> {
    // input path from --profile-output
    fs_util::create_dir_if_not_exists(output).categorize_input()?;

    fs_util::write(
        output.join("targets.txt"),
        profile_data
            .targets
            .iter()
            .map(|t| format!("{t}\n"))
            .collect::<String>(),
    )
    .categorize_internal()
    .yak_error_context("Failed to write targets")?;

    if let Some(profile) = profile_data.profile_data.gen_flame_data()? {
        let mut options = inferno::flamegraph::Options::default();
        let title = format!("Flame Graph - {}", profile_data.profile_data.profile_mode());
        options.title = if targets.len() == 1 {
            format!("{} on {}", title, targets[0])
        } else if targets.len() > 1 {
            format!("{} on {} and {} more", title, targets[0], targets.len() - 1)
        } else {
            title
        };

        write_starlark_flamegraph(profile, &output.join("flame"), options)?;
    }

    match profile_data.profile_data.profile_mode() {
        ProfileMode::HeapFlameAllocated | ProfileMode::HeapFlameRetained => {}
        _ => {
            let profile = profile_data.profile_data.gen_csv()?;
            fs_util::write(output.join("profile.csv"), profile)
                .categorize_internal()
                .yak_error_context("Failed to write profile")?;
        }
    };
    Ok(())
}

/// Will write the flamegraph profile to `<output_prefix.src` and `<output_prefix>.svg`
pub fn write_starlark_flamegraph(
    mut profile: String,
    output_prefix: &AbsPath,
    mut options: inferno::flamegraph::Options,
) -> yak_error::Result<()> {
    if profile.is_empty() {
        // inferno does not like empty flamegraphs.
        profile = "empty 1\n".to_owned();
    }
    let mut svg = Vec::new();

    inferno::flamegraph::from_reader(&mut options, profile.as_bytes(), &mut svg)
        .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::Profile))
        .yak_error_context("writing SVG from profile data")?;

    let src_path = output_prefix.with_added_extension("src");
    fs_util::write(&src_path, &profile)
        .categorize_internal()
        .yak_error_context(format!("Failed to write {src_path}"))?;
    let svg_path = output_prefix.with_added_extension("svg");
    fs_util::write(&svg_path, &svg)
        .categorize_internal()
        .yak_error_context(format!("Failed to write {svg_path}"))?;

    Ok(())
}

pub fn get_profile_response(
    profile_data: Arc<StarlarkProfileDataAndStats>,
    targets: &[String],
    output: &AbsPath,
) -> yak_error::Result<yak_cli_proto::ProfileResponse> {
    write_starlark_profile(profile_data.as_ref(), targets, output)?;

    Ok(yak_cli_proto::ProfileResponse {
        elapsed: Some(profile_data.duration().try_into()?),
        total_retained_bytes: profile_data.total_retained_bytes() as u64,
    })
}
