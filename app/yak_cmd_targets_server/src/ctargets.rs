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
use dice::DiceTransaction;
use yak_build_api::configure_targets::load_compatible_patterns_with_modifiers;
use yak_cli_proto::ConfiguredTargetsRequest;
use yak_cli_proto::ConfiguredTargetsResponse;
use yak_common::pattern::parse_from_cli::parse_patterns_with_modifiers_from_cli_args;
use yak_core::pattern::pattern_type::TargetPatternExtra;
use yak_error::YakErrorOptionContext;
use yak_node::load_patterns::MissingTargetBehavior;
use yak_node::nodes::configured::ConfiguredTargetNode;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::global_cfg_options::global_cfg_options_from_client_context;
use yak_server_ctx::partial_result_dispatcher::NoPartialResult;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use yak_server_ctx::template::ServerCommandTemplate;
use yak_server_ctx::template::run_server_command;

use crate::configured_target_hash::ConfiguredTargetHashOptions;
use crate::configured_target_hash::ConfiguredTargetHashes;
use crate::targets::fmt::ConfiguredOutputHandler;
use crate::targets::fmt::create_configured_formatter;

pub async fn configured_targets_command(
    server_ctx: &dyn ServerCommandContextTrait,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: ConfiguredTargetsRequest,
) -> yak_error::Result<ConfiguredTargetsResponse> {
    run_server_command(
        ConfiguredTargetsServerCommand { req },
        server_ctx,
        partial_result_dispatcher,
    )
    .await
}

struct ConfiguredTargetsServerCommand {
    req: ConfiguredTargetsRequest,
}

#[async_trait]
impl ServerCommandTemplate for ConfiguredTargetsServerCommand {
    type StartEvent = yak_data::ConfiguredTargetsCommandStart;
    type EndEvent = yak_data::ConfiguredTargetsCommandEnd;
    type Response = ConfiguredTargetsResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        ctx: DiceTransaction,
    ) -> yak_error::Result<ConfiguredTargetsResponse> {
        // TODO(nga): this should accept `ConfiguredTargetPatternExtra`. And handle the universe.
        let parsed_patterns_with_modifiers =
            parse_patterns_with_modifiers_from_cli_args::<TargetPatternExtra>(
                &mut ctx.ctx(),
                &self.req.target_patterns,
                server_ctx.working_dir(),
            )
            .await?;

        let output_handler = create_configured_formatter(&self.req)?;

        let global_cfg_options = global_cfg_options_from_client_context(
            self.req
                .target_cfg
                .as_ref()
                .internal_error("target_cfg must be set")?,
            server_ctx,
            &mut ctx.ctx(),
        )
        .await?;

        let skip_missing_targets = MissingTargetBehavior::from_skip(self.req.skip_missing_targets);

        let keep_going = self.req.keep_going;

        let result = load_compatible_patterns_with_modifiers(
            &mut ctx.ctx(),
            parsed_patterns_with_modifiers,
            &global_cfg_options,
            skip_missing_targets,
            keep_going,
        )
        .await?;

        let hashes = if self.req.show_target_hash {
            Some(ConfiguredTargetHashes::compute(
                &result.compatible_targets,
                &ConfiguredTargetHashOptions {
                    recursive: self.req.target_hash_recursive,
                    use_fast_hash: !self.req.target_hash_use_strong_hash,
                },
            )?)
        } else {
            None
        };

        let target_hash_for_node = |node: &ConfiguredTargetNode| match &hashes {
            Some(hashes) => hashes.get(node.label()).map(Some),
            None => Ok(None),
        };

        let mut serialized_targets_output = String::new();
        let mut stderr_output = String::new();

        match output_handler {
            ConfiguredOutputHandler::Formatter(formatter) => {
                formatter.begin(&mut serialized_targets_output);

                // Format errors
                let mut needs_separator = false;
                for error in result.errors {
                    if let Some(package_with_modifiers) = error.package {
                        // Package-level error
                        if needs_separator {
                            formatter.separator(&mut serialized_targets_output);
                        }
                        needs_separator = true;

                        let mut stderr_buf = String::new();
                        formatter.package_error(
                            package_with_modifiers.package,
                            &error.error,
                            &mut serialized_targets_output,
                            &mut stderr_buf,
                        );
                        stderr_output.push_str(&stderr_buf);
                    } else {
                        // Target-level error
                        if needs_separator {
                            formatter.separator(&mut serialized_targets_output);
                        }
                        needs_separator = true;

                        let mut stderr_buf = String::new();
                        formatter.target_error(
                            &error.error,
                            &mut serialized_targets_output,
                            &mut stderr_buf,
                        );
                        stderr_output.push_str(&stderr_buf);
                    }
                }

                // Format compatible targets
                for node in &result.compatible_targets {
                    let nodes = std::iter::once(node).chain(node.forward_target());
                    for node in nodes {
                        if needs_separator {
                            formatter.separator(&mut serialized_targets_output);
                        }
                        needs_separator = true;
                        formatter.target(
                            node,
                            target_hash_for_node(node)?,
                            &mut serialized_targets_output,
                        )?;
                    }
                }

                formatter.end(&mut serialized_targets_output);
            }
            ConfiguredOutputHandler::JsonReport(json_report_formatter) => {
                json_report_formatter.format_report(
                    &result,
                    target_hash_for_node,
                    &mut serialized_targets_output,
                    &mut stderr_output,
                )?;
            }
        }

        // Print errors to stderr if exists
        if !stderr_output.is_empty() {
            server_ctx.stderr()?.write_all(stderr_output.as_bytes())?;
        }

        Ok(ConfiguredTargetsResponse {
            serialized_targets_output,
        })
    }
}
