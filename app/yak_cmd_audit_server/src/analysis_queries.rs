/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::Write;

use async_trait::async_trait;
use dupe::Dupe;
use yak_analysis::analysis::calculation::resolve_queries;
use yak_cli_proto::ClientContext;
use yak_cmd_audit_client::analysis_queries::AuditAnalysisQueriesCommand;
use yak_common::pattern::parse_from_cli::parse_and_resolve_patterns_from_cli_args;
use yak_core::pattern::pattern_type::TargetPatternExtra;
use yak_core::target::label::label::TargetLabel;
use yak_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::ctx::ServerCommandDiceContext;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;
use crate::common::target_resolution_config::audit_command_target_resolution_config;

#[async_trait]
impl ServerAuditSubcommand for AuditAnalysisQueriesCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        mut stdout: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> yak_error::Result<()> {
        Ok(server_ctx
            .with_dice_ctx(|server_ctx, ctx| async move {
                let target_resolution_config = audit_command_target_resolution_config(
                    &mut ctx.ctx(),
                    &self.target_cfg,
                    server_ctx,
                )
                .await?;

                let resolved_pattern =
                    parse_and_resolve_patterns_from_cli_args::<TargetPatternExtra>(
                        &mut ctx.ctx(),
                        &self.patterns,
                        server_ctx.working_dir(),
                    )
                    .await?;

                let mut stdout = stdout.as_writer();

                for (package_with_modifiers, spec) in resolved_pattern.specs {
                    match spec {
                        yak_core::pattern::pattern::PackageSpec::Targets(targets) => {
                            for (target, TargetPatternExtra) in targets {
                                let label = TargetLabel::new(
                                    package_with_modifiers.package,
                                    target.as_ref(),
                                );
                                for configured_target in target_resolution_config
                                    .get_configured_target(&mut ctx.ctx(), &label, None)
                                    .await?
                                {
                                    let node = ctx
                                        .ctx()
                                        .get_configured_target_node(&configured_target)
                                        .await
                                        .require_compatible()?
                                        .dupe();

                                    let query_results =
                                        resolve_queries(&mut ctx.ctx(), node.as_ref()).await?;
                                    writeln!(stdout, "{label}:")?;
                                    for (query, result) in &query_results {
                                        writeln!(stdout, "  {query}")?;
                                        for (target, providers) in &result.result {
                                            writeln!(stdout, "    {}", target.unconfigured())?;
                                            if self.include_outputs {
                                                let outputs = providers
                                                    .provider_collection()
                                                    .default_info()?
                                                    .default_outputs_raw();
                                                writeln!(stdout, "        {outputs}")?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        yak_core::pattern::pattern::PackageSpec::All() => {
                            return Err(yak_error::yak_error!(
                                yak_error::ErrorTag::Unimplemented,
                                "PackageSpec::All not implemented"
                            ));
                        }
                    }
                }

                Ok(())
            })
            .await?)
    }
}
