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
use yak_cli_proto::new_generic::AnonTargetsRequest;
use yak_cli_proto::new_generic::AnonTargetsResponse;
use yak_cli_proto::new_generic::CompleteRequest;
use yak_cli_proto::new_generic::CompleteResponse;
use yak_cli_proto::new_generic::DebugEvalRequest;
use yak_cli_proto::new_generic::DebugEvalResponse;
use yak_cli_proto::new_generic::ExpandExternalCellsRequest;
use yak_cli_proto::new_generic::ExpandExternalCellsResponse;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::late_bindings::OTHER_SERVER_COMMANDS;
use yak_server_ctx::late_bindings::OtherServerCommands;
use yak_server_ctx::partial_result_dispatcher::NoPartialResult;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::build::build_command;
use crate::complete::complete_command;
use crate::debug_anon_targets::debug_anon_targets_command;
use crate::debug_eval::debug_eval_command;
use crate::expand_external_cells::expand_external_cells_command;
use crate::install::install_command;

struct OtherServerCommandsInstance;

#[async_trait]
impl OtherServerCommands for OtherServerCommandsInstance {
    async fn build(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::BuildRequest,
    ) -> yak_error::Result<yak_cli_proto::BuildResponse> {
        build_command(ctx, partial_result_dispatcher, req).await
    }
    async fn install(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::InstallRequest,
    ) -> yak_error::Result<yak_cli_proto::InstallResponse> {
        install_command(ctx, partial_result_dispatcher, req).await
    }
    async fn complete(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: CompleteRequest,
    ) -> yak_error::Result<CompleteResponse> {
        complete_command(ctx, partial_result_dispatcher, req).await
    }

    async fn debug_eval(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        req: DebugEvalRequest,
    ) -> yak_error::Result<DebugEvalResponse> {
        debug_eval_command(ctx, req).await
    }

    async fn debug_anon_targets(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        req: AnonTargetsRequest,
    ) -> yak_error::Result<AnonTargetsResponse> {
        debug_anon_targets_command(ctx, req).await
    }

    async fn expand_external_cells(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: ExpandExternalCellsRequest,
    ) -> yak_error::Result<ExpandExternalCellsResponse> {
        expand_external_cells_command(ctx, partial_result_dispatcher, req).await
    }
}

pub(crate) fn init_other_server_commands() {
    OTHER_SERVER_COMMANDS.init(&OtherServerCommandsInstance);
}
