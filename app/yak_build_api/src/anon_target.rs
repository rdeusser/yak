/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use dice::DiceComputations;
use dupe::Dupe;
use starlark::collections::SmallMap;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;
use starlark::values::ValueOfUnchecked;
use starlark::values::structs::StructRef;
use yak_artifact::artifact::artifact_type::Artifact;
use yak_core::deferred::base_deferred_key::BaseDeferredKey;
use yak_core::deferred::base_deferred_key::BaseDeferredKeyDyn;
use yak_core::execution_types::execution::ExecutionPlatformResolution;
use yak_core::target::configured_target_label::ConfiguredTargetLabel;
use yak_hash::StdYakHashMap;
use yak_hash::YakMutMap;
use yak_interpreter::dice::starlark_provider::StarlarkEvalKind;
use yak_node::attrs::spec::AttributeSpec;
use yak_node::rule_type::StarlarkRuleType;
use yak_util::late_binding::LateBinding;

use crate::analysis::AnalysisResult;
use crate::artifact_groups::promise::PromiseArtifactAttr;
use crate::artifact_groups::promise::PromiseArtifactId;
use crate::validation::transitive_validations::TransitiveValidations;

pub trait AnonTargetDyn: Send + Sync + Display {
    fn eval_kind(self: Arc<Self>) -> StarlarkEvalKind;

    fn rule_type(&self) -> &Arc<StarlarkRuleType>;

    fn base_deferred_key(self: Arc<Self>) -> BaseDeferredKey;

    fn get_fulfilled_promise_artifacts<'v>(
        self: Arc<Self>,
        promise_artifact_mappings: SmallMap<String, Value<'v>>,
        anon_target_result: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> yak_error::Result<StdYakHashMap<PromiseArtifactId, Artifact>>;

    /// `attrs_spec` must be the spec of the rule this anon target was
    /// defined with: attribute names are not stored in the target and are
    /// recovered from the spec.
    fn resolve_attrs<'v>(
        &self,
        env: &Module<'v>,
        attrs_spec: &AttributeSpec,
        dependents_analyses: AnonTargetDependentAnalysisResults<'_>,
        exec_resolution: ExecutionPlatformResolution,
    ) -> yak_error::Result<ValueOfUnchecked<'v, StructRef<'static>>>;
}

/// Rendered description of an anon target, for introspection commands.
#[derive(Debug, serde::Serialize)]
pub struct AnonTargetNodeInfo {
    /// The anon target's name (a target label in the synthetic `anon` cell).
    pub name: String,
    /// The package of `name`.
    pub package: String,
    /// Hash identifying the key (rule + attrs + execution configuration).
    pub hash: String,
    pub rule_type: String,
    /// `"bzl"` or `"bxl"`.
    pub variant: &'static str,
    pub execution_configuration: String,
    /// Attr name to rendered attr value.
    pub attrs: serde_json::Value,
    /// Configured targets this anon target depends on through its attrs.
    pub deps: Vec<String>,
    /// Anon targets whose promise artifacts appear in this target's attrs.
    pub promise_artifact_deps: Vec<String>,
}

/// Renders the `BaseDeferredKey::AnonTarget` key of an anon target.
///
/// Implemented in `yak_anon_target`, where the key can be downcast; DICE is needed to
/// load the rule's `AttributeSpec`, which holds the attr names.
pub static GET_ANON_TARGET_NODE_INFO: LateBinding<
    for<'c, 'd> fn(
        &'c mut DiceComputations<'d>,
        Arc<dyn BaseDeferredKeyDyn>,
    ) -> Pin<
        Box<dyn Future<Output = yak_error::Result<AnonTargetNodeInfo>> + Send + 'c>,
    >,
> = LateBinding::new("GET_ANON_TARGET_NODE_INFO");

// Container for analysis results of the anon target dependents.
pub struct AnonTargetDependentAnalysisResults<'v> {
    pub dep_analysis_results: Vec<(&'v ConfiguredTargetLabel, AnalysisResult)>,
    pub promised_artifacts: YakMutMap<&'v PromiseArtifactAttr, Artifact>,
}

impl<'v> AnonTargetDependentAnalysisResults<'v> {
    pub fn validations(&self) -> SmallMap<ConfiguredTargetLabel, TransitiveValidations> {
        self.dep_analysis_results
            .iter()
            .filter_map(|(label, analysis_result)| {
                analysis_result
                    .validations
                    .dupe()
                    .map(|v| ((*label).dupe(), v))
            })
            .collect::<SmallMap<_, _>>()
    }
}
