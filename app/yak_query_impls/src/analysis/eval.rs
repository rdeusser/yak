/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_build_api::analysis::calculation::EVAL_ANALYSIS_QUERY;
use yak_hash::YakMutMap;
use yak_node::nodes::configured::ConfiguredTargetNode;
use yak_node::nodes::configured_ref::ConfiguredGraphNodeRef;
use yak_query::query::syntax::simple::eval::evaluator::QueryEvaluator;
use yak_query::query::syntax::simple::eval::set::TargetSet;

use crate::analysis::configured_graph::AnalysisConfiguredGraphQueryDelegate;
use crate::analysis::environment::ConfiguredGraphQueryEnvironment;

pub(crate) fn init_eval_analysis_query() {
    EVAL_ANALYSIS_QUERY
        .init(|query, resolved_literals| Box::pin(eval_analysis_query(query, resolved_literals)));
}

async fn eval_analysis_query(
    query: &str,
    resolved_literals: YakMutMap<String, ConfiguredTargetNode>,
) -> yak_error::Result<TargetSet<ConfiguredGraphNodeRef>> {
    let delegate = AnalysisConfiguredGraphQueryDelegate { resolved_literals };
    let functions = ConfiguredGraphQueryEnvironment::functions();
    let env = ConfiguredGraphQueryEnvironment::new(&delegate);
    let evaluator = QueryEvaluator::new(&env, &functions);

    let result = evaluator.eval_query(query).await?;
    result.try_into_targets()
}
