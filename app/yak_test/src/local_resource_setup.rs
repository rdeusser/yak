/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dice::DiceComputations;
use itertools::Itertools;
use yak_build_api::analysis::calculation::RuleAnalysisCalculation;
use yak_build_api::interpreter::rule_defs::provider::builtin::local_resource_info::LocalResourceInfo;
use yak_build_api::interpreter::rule_defs::provider::builtin::local_resource_info::OwnedLocalResourceInfo;
use yak_core::provider::label::ConfiguredProvidersLabel;
use yak_core::soft_error;
use yak_core::target::configured_target_label::ConfiguredTargetLabel;
use yak_error::ErrorTag;
use yak_error::internal_error;
use yak_hash::BuckMutMap;
use yak_test_api::data::RequiredLocalResources;
use yak_test_api::data::TestStage;

pub(crate) enum TestStageSimple {
    Listing,
    Testing,
}

impl From<&TestStage> for TestStageSimple {
    fn from(value: &TestStage) -> Self {
        match value {
            TestStage::Listing { .. } => TestStageSimple::Listing,
            TestStage::Testing { .. } => TestStageSimple::Testing,
        }
    }
}

pub(crate) async fn required_providers<'v>(
    dice: &mut DiceComputations<'_>,
    available_resources: BuckMutMap<&'v str, Option<&'v ConfiguredProvidersLabel>>,
    rule_required_resource_names: Vec<&'v str>,
    required_local_resources: &'v RequiredLocalResources,
) -> yak_error::Result<Vec<(&'v ConfiguredTargetLabel, OwnedLocalResourceInfo)>> {
    let targets = required_local_resources
        .resources
        .iter()
        .map(|resource_type| &resource_type.name as &'v str)
        .chain(rule_required_resource_names)
        .unique()
        .map(|type_name| {
            available_resources.get(type_name).copied().ok_or_else(|| {
                yak_error::yak_error!(
                    ErrorTag::Input,
                    "Required local resource of type `{type_name}` not found.",
                )
            })
        })
        .filter_map(|r| match r {
            Ok(Some(x)) => Some(Ok(x)),
            Ok(None) => None,
            Err(e) => {
                let _ignore = soft_error!("missing_required_local_resource", e, quiet: true, hard_error: true);
                None
            }
        })
        .collect::<Result<Vec<_>, yak_error::Error>>()?;

    dice.compute_join(targets, async |dice, target| {
        get_local_resource_info(dice, target).await
    })
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
}

async fn get_local_resource_info<'v>(
    dice: &mut DiceComputations<'_>,
    target: &'v ConfiguredProvidersLabel,
) -> yak_error::Result<(&'v ConfiguredTargetLabel, OwnedLocalResourceInfo)> {
    let local_resource_info = dice
        .get_providers(target)
        .await?
        .require_compatible()?
        .builtin_provider_value::<LocalResourceInfo>()
        .ok_or_else(|| {
            internal_error!("Target `{target}` expected to contain `LocalResourceInfo` provider")
        })?;
    Ok((target.target(), local_resource_info))
}
