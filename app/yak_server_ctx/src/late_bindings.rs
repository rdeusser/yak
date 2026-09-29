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
use yak_cli_proto::new_generic::CompleteRequest;
use yak_cli_proto::new_generic::CompleteResponse;
use yak_cli_proto::new_generic::DebugEvalRequest;
use yak_cli_proto::new_generic::DebugEvalResponse;
use yak_cli_proto::new_generic::ExpandExternalCellsRequest;
use yak_cli_proto::new_generic::ExpandExternalCellsResponse;
use yak_util::late_binding::LateBinding;

use crate::ctx::ServerCommandContextTrait;
use crate::partial_result_dispatcher::NoPartialResult;
use crate::partial_result_dispatcher::PartialResultDispatcher;

#[async_trait]
pub trait OtherServerCommands: Send + Sync + 'static {
    async fn build(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::BuildRequest,
    ) -> yak_error::Result<yak_cli_proto::BuildResponse>;
    async fn install(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::InstallRequest,
    ) -> yak_error::Result<yak_cli_proto::InstallResponse>;
    async fn complete(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: CompleteRequest,
    ) -> yak_error::Result<CompleteResponse>;
    async fn debug_eval(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        req: DebugEvalRequest,
    ) -> yak_error::Result<DebugEvalResponse>;
    async fn expand_external_cells(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: ExpandExternalCellsRequest,
    ) -> yak_error::Result<ExpandExternalCellsResponse>;
}

pub static OTHER_SERVER_COMMANDS: LateBinding<&'static dyn OtherServerCommands> =
    LateBinding::new("OTHER_SERVER_COMMANDS");

#[async_trait]
pub trait TargetsServerCommands: Send + Sync + 'static {
    async fn targets(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::TargetsRequest,
    ) -> yak_error::Result<yak_cli_proto::TargetsResponse>;
    async fn targets_show_outputs(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::TargetsRequest,
    ) -> yak_error::Result<yak_cli_proto::TargetsShowOutputsResponse>;
    async fn ctargets(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::ConfiguredTargetsRequest,
    ) -> yak_error::Result<yak_cli_proto::ConfiguredTargetsResponse>;
}

pub static TARGETS_SERVER_COMMANDS: LateBinding<&'static dyn TargetsServerCommands> =
    LateBinding::new("TARGETS_SERVER_COMMANDS");

#[async_trait]
pub trait QueryServerCommands: Send + Sync + 'static {
    async fn uquery(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::UqueryRequest,
    ) -> yak_error::Result<yak_cli_proto::UqueryResponse>;
    async fn cquery(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::CqueryRequest,
    ) -> yak_error::Result<yak_cli_proto::CqueryResponse>;
    async fn aquery(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::AqueryRequest,
    ) -> yak_error::Result<yak_cli_proto::AqueryResponse>;
}

pub static QUERY_SERVER_COMMANDS: LateBinding<&'static dyn QueryServerCommands> =
    LateBinding::new("QUERY_SERVER_COMMANDS");

#[async_trait]
pub trait DocsServerCommand: Send + Sync + 'static {
    async fn docs(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: yak_cli_proto::new_generic::DocsRequest,
    ) -> yak_error::Result<yak_cli_proto::new_generic::DocsResponse>;
}

pub static DOCS_SERVER_COMMAND: LateBinding<&'static dyn DocsServerCommand> =
    LateBinding::new("DOCS_SERVER_COMMAND");

#[async_trait]
pub trait AuditServerCommand: Send + Sync + 'static {
    async fn audit(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::GenericRequest,
    ) -> yak_error::Result<yak_cli_proto::GenericResponse>;
}

pub static AUDIT_SERVER_COMMAND: LateBinding<&'static dyn AuditServerCommand> =
    LateBinding::new("AUDIT_SERVER_COMMAND");

#[async_trait]
pub trait StarlarkServerCommand: Send + Sync + 'static {
    async fn starlark(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<yak_cli_proto::StdoutBytes>,
        req: yak_cli_proto::GenericRequest,
    ) -> yak_error::Result<yak_cli_proto::GenericResponse>;
}

pub static STARLARK_SERVER_COMMAND: LateBinding<&'static dyn StarlarkServerCommand> =
    LateBinding::new("STARLARK_SERVER_COMMAND");
