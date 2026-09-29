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
use dupe::Dupe;
use dupe::IterDupedExt;
use yak_core::configuration::compatibility::MaybeCompatible;
use yak_node::nodes::configured::ConfiguredTargetNode;
use yak_node::nodes::configured_node_ref::ConfiguredTargetNodeRefNode;
use yak_node::nodes::configured_node_ref::ConfiguredTargetNodeRefNodeDeps;
use yak_node::nodes::configured_ref::ConfiguredGraphNodeRef;
use yak_node::query::query_functions::CONFIGURED_GRAPH_QUERY_FUNCTIONS;
use yak_query::query::environment::QueryEnvironment;
use yak_query::query::environment::TraversalFilter;
use yak_query::query::environment::deps;
use yak_query::query::graph::dfs::dfs_postorder;
use yak_query::query::graph::successors::AsyncChildVisitor;
use yak_query::query::syntax::simple::eval::error::QueryError;
use yak_query::query::syntax::simple::eval::file_set::FileSet;
use yak_query::query::syntax::simple::eval::set::TargetSet;
use yak_query::query::syntax::simple::eval::values::QueryValueDepth;
use yak_query::query::syntax::simple::functions::DefaultQueryFunctionsModule;
use yak_query::query::traversal::NodeLookupId;
use yak_query::query::traversal::async_depth_limited_traversal;
use yak_query::query::traversal::async_fast_depth_first_postorder_traversal;

#[derive(Debug, yak_error::Error)]
#[yak(tag = Input)]
enum AnalysisQueryError {
    #[error("file literals aren't supported in query attributes (got `{0}`)")]
    FileLiteralsNotAllowed(String),
}

pub(crate) trait ConfiguredGraphQueryEnvironmentDelegate: Send + Sync {
    fn eval_literal(&self, literal: &str) -> yak_error::Result<ConfiguredTargetNode>;
}

pub(crate) struct ConfiguredGraphQueryEnvironment<'a> {
    delegate: &'a dyn ConfiguredGraphQueryEnvironmentDelegate,
}

impl<'a> ConfiguredGraphQueryEnvironment<'a> {
    pub(crate) fn new(delegate: &'a dyn ConfiguredGraphQueryEnvironmentDelegate) -> Self {
        Self { delegate }
    }

    pub(crate) fn functions() -> DefaultQueryFunctionsModule<ConfiguredGraphQueryEnvironment<'a>> {
        DefaultQueryFunctionsModule::new()
    }
}

pub(crate) fn init_query_functions() {
    CONFIGURED_GRAPH_QUERY_FUNCTIONS.init(Arc::new(ConfiguredGraphQueryEnvironment::functions()));
}

#[async_trait]
impl QueryEnvironment for ConfiguredGraphQueryEnvironment<'_> {
    type Target = ConfiguredGraphNodeRef;

    async fn get_node(&self, node_ref: &ConfiguredGraphNodeRef) -> yak_error::Result<Self::Target> {
        Ok(node_ref.dupe())
    }

    async fn get_node_for_default_configured_target(
        &self,
        _node_ref: &ConfiguredGraphNodeRef,
    ) -> yak_error::Result<MaybeCompatible<Self::Target>> {
        Err(QueryError::FunctionUnimplemented(
            "get_node_for_default_configured_target() only for CqueryEnvironment",
        )
        .into())
    }

    async fn eval_literals(&self, literal: &[&str]) -> yak_error::Result<TargetSet<Self::Target>> {
        let mut result = TargetSet::new();
        for lit in literal {
            result.insert(ConfiguredGraphNodeRef::new(
                self.delegate.eval_literal(lit)?,
            ));
        }
        Ok(result)
    }

    async fn eval_file_literal(&self, literal: &str) -> yak_error::Result<FileSet> {
        Err(AnalysisQueryError::FileLiteralsNotAllowed(literal.to_owned()).into())
    }

    async fn dfs_postorder(
        &self,
        root: &TargetSet<Self::Target>,
        delegate: impl AsyncChildVisitor<Self::Target>,
        visit: impl FnMut(Self::Target) -> yak_error::Result<()> + Send,
    ) -> yak_error::Result<()> {
        async_fast_depth_first_postorder_traversal(
            &NodeLookupId,
            root.iter().duped(),
            delegate,
            visit,
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
        async_depth_limited_traversal(
            &NodeLookupId,
            root.iter(),
            delegate,
            visit,
            depth,
            self.allow_partial_graph(),
        )
        .await
    }

    async fn owner(&self, _paths: &FileSet) -> yak_error::Result<TargetSet<Self::Target>> {
        Err(QueryError::FunctionUnimplemented("owner").into())
    }

    async fn targets_in_buildfile(
        &self,
        _paths: &FileSet,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        Err(QueryError::FunctionUnimplemented("targets_in_buildfile").into())
    }

    async fn deps(
        &self,
        targets: &TargetSet<Self::Target>,
        depth: QueryValueDepth,
        filter: Option<&dyn TraversalFilter<Self::Target>>,
    ) -> yak_error::Result<TargetSet<Self::Target>> {
        if depth.is_unbounded() && filter.is_none() {
            // TODO(nga): fast lookup with depth too.
            let mut deps: TargetSet<Self::Target> = TargetSet::new();
            dfs_postorder::<ConfiguredTargetNodeRefNode>(
                targets.iter().map(|n| ConfiguredTargetNodeRefNode::new(n)),
                ConfiguredTargetNodeRefNodeDeps,
                |target| {
                    deps.insert(ConfiguredGraphNodeRef::new(target.to_node()));
                    Ok(())
                },
            )?;
            Ok(deps)
        } else {
            deps(self, targets, depth, filter).await
        }
    }
}
