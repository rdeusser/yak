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

use async_trait::async_trait;
use dice::DiceComputations;
use yak_artifact::actions::key::ActionKey;
use yak_build_api::actions::query::ActionQueryNode;
use yak_build_api::actions::query::ActionQueryNodeRef;
use yak_build_api::actions::query::SetProjectionInputs;
use yak_build_api::analysis::AnalysisResult;
use yak_build_api::artifact_groups::ArtifactGroup;
use yak_build_api::artifact_groups::TransitiveSetProjectionKey;
use yak_core::configuration::compatibility::MaybeCompatible;
use yak_core::provider::label::ConfiguredProvidersLabel;
use yak_query::query::environment::QueryEnvironment;
use yak_query::query::environment::TraversalFilter;
use yak_query::query::environment::deps;
use yak_query::query::environment::rdeps;
use yak_query::query::environment::somepath;
use yak_query::query::graph::successors::AsyncChildVisitor;
use yak_query::query::syntax::simple::eval::error::QueryError;
use yak_query::query::syntax::simple::eval::file_set::FileSet;
use yak_query::query::syntax::simple::eval::set::TargetSet;
use yak_query::query::syntax::simple::eval::values::QueryValueDepth;
use yak_query::query::syntax::simple::functions::DefaultQueryFunctionsModule;
use yak_query::query::syntax::simple::functions::HasModuleDescription;
use yak_query::query::syntax::simple::functions::docs::QueryEnvironmentDescription;
use yak_query::query::traversal::AsyncNodeLookup;
use yak_query::query::traversal::async_depth_first_postorder_traversal;
use yak_query::query::traversal::async_depth_limited_traversal;

use crate::aquery::functions::AqueryFunctions;
use crate::aquery::mixed_graph::aquery_deps_bounded_unfiltered;
use crate::aquery::mixed_graph::aquery_deps_unbounded_unfiltered;
use crate::aquery::mixed_graph::aquery_rdeps_unfiltered;
use crate::aquery::mixed_graph::aquery_somepath_unfiltered;
use crate::cquery::environment::CqueryDelegate;
use crate::uquery::environment::QueryLiterals;

/// CqueryDelegate resolves information needed by the QueryEnvironment.
#[async_trait]
pub(crate) trait AqueryDelegate: Send + Sync {
    fn cquery_delegate(&self) -> &dyn CqueryDelegate;

    fn ctx(&self) -> DiceComputations<'_>;

    async fn get_node(&self, key: &ActionKey) -> yak_error::Result<ActionQueryNode>;

    async fn get_tset_node(
        &self,
        key: &TransitiveSetProjectionKey,
    ) -> yak_error::Result<SetProjectionInputs>;

    async fn expand_artifacts(
        &self,
        artifacts: &[ArtifactGroup],
    ) -> yak_error::Result<Vec<ActionQueryNode>>;

    async fn get_target_set_from_analysis(
        &self,
        configured_label: &ConfiguredProvidersLabel,
        analysis: AnalysisResult,
    ) -> yak_error::Result<TargetSet<ActionQueryNode>>;
}

pub(crate) struct AqueryEnvironment<'c> {
    pub(super) delegate: Arc<dyn AqueryDelegate + 'c>,
    literals: Arc<dyn QueryLiterals<ActionQueryNode> + 'c>,
}

impl<'c> AqueryEnvironment<'c> {
    pub(crate) fn new(
        delegate: Arc<dyn AqueryDelegate + 'c>,
        literals: Arc<dyn QueryLiterals<ActionQueryNode> + 'c>,
    ) -> Self {
        Self { delegate, literals }
    }

    async fn get_node(&self, label: &ActionQueryNodeRef) -> yak_error::Result<ActionQueryNode> {
        // We do not allow traversing edges in targets in aquery
        self.delegate.get_node(label.require_action()?).await
    }

    pub(crate) fn describe() -> QueryEnvironmentDescription {
        QueryEnvironmentDescription {
            name: "Aquery Environment".to_owned(),
            mods: vec![
                DefaultQueryFunctionsModule::<Self>::describe(),
                AqueryFunctions::describe(),
            ],
        }
    }
}

#[async_trait]
impl QueryEnvironment for AqueryEnvironment<'_> {
    type Target = ActionQueryNode;

    async fn get_node(&self, node_ref: &ActionQueryNodeRef) -> yak_error::Result<Self::Target> {
        AqueryEnvironment::get_node(self, node_ref).await
    }

    async fn get_node_for_default_configured_target(
        &self,
        _node_ref: &ActionQueryNodeRef,
    ) -> yak_error::Result<MaybeCompatible<Self::Target>> {
        Err(QueryError::FunctionUnimplemented(
            "get_node_for_default_configured_target() only for CqueryEnvironment",
        )
        .into())
    }

    async fn eval_literals(&self, literals: &[&str]) -> yak_error::Result<TargetSet<Self::Target>> {
        self.literals
            .eval_literals(literals, &mut self.delegate.ctx())
            .await
    }

    async fn eval_file_literal(&self, literal: &str) -> yak_error::Result<FileSet> {
        self.delegate
            .cquery_delegate()
            .uquery_delegate()
            .eval_file_literal(literal)
            .await
    }

    // Unfiltered `deps`/`rdeps`/`somepath` (and thereby `allpaths`) traverse the mixed
    // action/tset graph in O(nodes + edges) instead of paying for the flattened tset
    // structure (see the `mixed_graph` module). Filtered forms fall back to the flattened
    // traversals, since filter expressions observe each node's flattened first-order deps.

    async fn deps(
        &self,
        targets: &TargetSet<Self::Target>,
        depth: QueryValueDepth,
        filter: Option<&dyn TraversalFilter<Self::Target>>,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        match (depth, &filter) {
            (QueryValueDepth::Unbounded, None) => {
                aquery_deps_unbounded_unfiltered(self, targets).await
            }
            (QueryValueDepth::Bounded(depth), None) => {
                aquery_deps_bounded_unfiltered(self, targets, depth).await
            }
            _ => deps(self, targets, depth, filter).await,
        }
    }

    async fn rdeps(
        &self,
        universe: &TargetSet<Self::Target>,
        from: &TargetSet<Self::Target>,
        depth: QueryValueDepth,
        filter: Option<&dyn TraversalFilter<Self::Target>>,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        match filter {
            None => aquery_rdeps_unfiltered(self, universe, from, depth.bound()).await,
            Some(_) => rdeps(self, universe, from, depth, filter).await,
        }
    }

    async fn somepath(
        &self,
        from: &TargetSet<Self::Target>,
        to: &TargetSet<Self::Target>,
        filter: Option<&dyn TraversalFilter<Self::Target>>,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        match filter {
            None => aquery_somepath_unfiltered(self, from, to).await,
            Some(_) => somepath(self, from, to, filter).await,
        }
    }

    async fn dfs_postorder(
        &self,
        root: &TargetSet<Self::Target>,
        traversal_delegate: impl AsyncChildVisitor<Self::Target>,
        visit: impl FnMut(Self::Target) -> yak_error::Result<()> + Send,
    ) -> yak_error::Result<()> {
        // This is now reached only by filtered traversals, which are inherently bound to the
        // flattened `deps()` view (the filter observes each node's flattened first-order deps)
        // and therefore pay its O(n^2) cost; unfiltered traversals use the `mixed_graph`
        // module's O(n + e) implementations instead.
        async_depth_first_postorder_traversal(
            &AqueryNodeLookup {
                roots: root,
                env: self,
            },
            root.iter_names(),
            traversal_delegate,
            visit,
            self.allow_partial_graph(),
        )
        .await
    }

    async fn depth_limited_traversal(
        &self,
        root: &TargetSet<Self::Target>,
        delegate: impl AsyncChildVisitor<Self::Target>,
        visit: impl FnMut(Self::Target) -> yak_error::Result<()> + Send,
        depth: u32,
    ) -> yak_error::Result<()> {
        // TODO(cjhopman): See above.
        async_depth_limited_traversal(
            &AqueryNodeLookup {
                roots: root,
                env: self,
            },
            root.iter_names(),
            delegate,
            visit,
            depth,
            self.allow_partial_graph(),
        )
        .await
    }

    async fn owner(&self, _paths: &FileSet) -> yak_error::Result<TargetSet<Self::Target>> {
        Err(QueryError::NotAvailableInContext("owner").into())
    }

    async fn targets_in_buildfile(
        &self,
        _paths: &FileSet,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        Err(QueryError::NotAvailableInContext("targets_in_buildfile").into())
    }
}

struct AqueryNodeLookup<'a, 'c> {
    roots: &'a TargetSet<ActionQueryNode>,
    env: &'a AqueryEnvironment<'c>,
}

#[async_trait]
impl AsyncNodeLookup<ActionQueryNode> for AqueryNodeLookup<'_, '_> {
    async fn get(&self, label: &ActionQueryNodeRef) -> yak_error::Result<ActionQueryNode> {
        // Lookup the node in `roots` first since `env.get_node` isn't capable of looking up
        // analysis nodes, and while won't find new analysis nodes while doing a DFS, we might pass
        // in roots that *are* analysis nodes.
        if let Some(v) = self.roots.get(label) {
            return Ok(v.clone());
        }
        self.env.get_node(label).await
    }
}
