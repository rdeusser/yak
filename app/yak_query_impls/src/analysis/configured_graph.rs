/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::OptionDupedExt;
use yak_hash::YakMutMap;
use yak_node::nodes::configured::ConfiguredTargetNode;

use crate::analysis::environment::ConfiguredGraphQueryEnvironmentDelegate;

pub(crate) struct AnalysisConfiguredGraphQueryDelegate {
    pub(crate) resolved_literals: YakMutMap<String, ConfiguredTargetNode>,
}

impl ConfiguredGraphQueryEnvironmentDelegate for AnalysisConfiguredGraphQueryDelegate {
    fn eval_literal(&self, literal: &str) -> yak_error::Result<ConfiguredTargetNode> {
        self.resolved_literals
            .get(literal)
            .duped()
            .ok_or_else(|| yak_error::yak_error!(yak_error::ErrorTag::Tier0, ""))
    }
}
