/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use buck2_hash::BuckMutMap;
use buck2_node::nodes::configured::ConfiguredTargetNode;
use dupe::OptionDupedExt;

use crate::analysis::environment::ConfiguredGraphQueryEnvironmentDelegate;

pub(crate) struct AnalysisConfiguredGraphQueryDelegate {
    pub(crate) resolved_literals: BuckMutMap<String, ConfiguredTargetNode>,
}

impl ConfiguredGraphQueryEnvironmentDelegate for AnalysisConfiguredGraphQueryDelegate {
    fn eval_literal(&self, literal: &str) -> buck2_error::Result<ConfiguredTargetNode> {
        self.resolved_literals
            .get(literal)
            .duped()
            .ok_or_else(|| buck2_error::buck2_error!(buck2_error::ErrorTag::Tier0, ""))
    }
}
