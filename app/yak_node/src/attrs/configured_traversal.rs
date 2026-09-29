/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_core::package::source_path::SourcePathRef;
use yak_core::plugins::PluginKind;
use yak_core::plugins::PluginKindSet;
use yak_core::provider::label::ConfiguredProvidersLabel;
use yak_core::provider::label::ProvidersLabel;
use yak_core::target::label::label::TargetLabel;

use crate::attrs::attr_type::query::ResolvedQueryLiterals;

pub trait ConfiguredAttrTraversal {
    fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> yak_error::Result<()>;

    fn dep_with_plugins(
        &mut self,
        dep: &ConfiguredProvidersLabel,
        _plugins: &PluginKindSet,
    ) -> yak_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn exec_dep(&mut self, dep: &ConfiguredProvidersLabel) -> yak_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn toolchain_dep(&mut self, dep: &ConfiguredProvidersLabel) -> yak_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn configuration_dep(&mut self, _dep: &ProvidersLabel) -> yak_error::Result<()> {
        Ok(())
    }

    fn plugin_dep(&mut self, _dep: &TargetLabel, _kind: &PluginKind) -> yak_error::Result<()> {
        Ok(())
    }

    /// Called for both `attrs.query(...)` and query macros like `$(query_targets ...)`.
    fn query(
        &mut self,
        _query: &str,
        _resolved_literals: &ResolvedQueryLiterals<ConfiguredProvidersLabel>,
    ) -> yak_error::Result<()> {
        Ok(())
    }

    fn input(&mut self, _path: SourcePathRef) -> yak_error::Result<()> {
        Ok(())
    }

    fn label(&mut self, _label: &ConfiguredProvidersLabel) -> yak_error::Result<()> {
        Ok(())
    }
}
