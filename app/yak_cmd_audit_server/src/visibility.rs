/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use dice::DiceTransaction;
use dupe::Dupe;
use futures::FutureExt;
use yak_cli_proto::ClientContext;
use yak_cmd_audit_client::visibility::AuditVisibilityCommand;
use yak_common::pattern::parse_from_cli::parse_patterns_from_cli_args;
use yak_core::pattern::pattern_type::TargetPatternExtra;
use yak_node::load_patterns::MissingTargetBehavior;
use yak_node::load_patterns::load_patterns;
use yak_node::nodes::lookup::TargetNodeLookup;
use yak_node::nodes::unconfigured::TargetNode;
use yak_query::query::environment::QueryTargetDepsSuccessors;
use yak_query::query::syntax::simple::eval::set::TargetSet;
use yak_query::query::traversal::async_depth_first_postorder_traversal;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::ctx::ServerCommandDiceContext;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Tier0)]
enum VisibilityCommandError {
    #[error(
        "Internal Error: The dependency `{0}` of the target `{1}` was not found during the traversal."
    )]
    DepNodeNotFound(String, String),
}

async fn verify_visibility(
    ctx: DiceTransaction,
    targets: TargetSet<TargetNode>,
) -> yak_error::Result<()> {
    let mut new_targets: TargetSet<TargetNode> = TargetSet::new();

    let visit = |target| {
        new_targets.insert(target);
        Ok(())
    };

    ctx.ctx()
        .with_linear_recompute(|ctx| {
            async move {
                let lookup = TargetNodeLookup(ctx);

                async_depth_first_postorder_traversal(
                    &lookup,
                    targets.iter_names(),
                    QueryTargetDepsSuccessors,
                    visit,
                    false, // allow_partial_graph
                )
                .await
            }
            .boxed()
        })
        .await?;

    let mut visibility_errors = Vec::new();

    for target in new_targets.iter() {
        for dep in target.deps() {
            match new_targets.get(dep) {
                Some(val) => {
                    if !val.is_visible_to(target.label())? {
                        visibility_errors.push(val.not_visible_to_error(target.label().dupe()));
                    }
                }
                None => {
                    return Err(yak_error::Error::from(
                        VisibilityCommandError::DepNodeNotFound(
                            dep.to_string(),
                            target.label().name().to_string(),
                        ),
                    ));
                }
            }
        }
    }

    for err in &visibility_errors {
        yak_client_ctx::eprintln!("{}", err)?;
    }

    if !visibility_errors.is_empty() {
        return Err(yak_error::yak_error!(yak_error::ErrorTag::Input, "{}", 1));
    }

    yak_client_ctx::eprintln!("audit visibility succeeded")?;
    Ok(())
}

#[async_trait]
impl ServerAuditSubcommand for AuditVisibilityCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _stdout: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> yak_error::Result<()> {
        Ok(server_ctx
            .with_dice_ctx(|server_ctx, ctx| async move {
                let parsed_patterns = parse_patterns_from_cli_args::<TargetPatternExtra>(
                    &mut ctx.ctx(),
                    &self.patterns,
                    server_ctx.working_dir(),
                )
                .await?;

                let parsed_target_patterns =
                    load_patterns(&mut ctx.ctx(), parsed_patterns, MissingTargetBehavior::Fail)
                        .await?;

                let mut nodes = TargetSet::<TargetNode>::new();
                for (_package, result) in parsed_target_patterns.iter() {
                    let res = result.as_ref().map_err(Dupe::dupe)?;
                    nodes.extend(res.values().map(|n| n.to_owned()));
                }

                verify_visibility(ctx, nodes).await?;
                Ok(())
            })
            .await?)
    }
}
