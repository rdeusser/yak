/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_cli_proto::trace_io_request;
use yak_cli_proto::trace_io_response;
use yak_common::file_ops::metadata::RawSymlink;
use yak_common::io::trace::TracingIoProvider;
use yak_error::BuckErrorContext;
use yak_events::dispatch::span_async;
use yak_execute::materialize::materializer::MaterializationPurpose;
use yak_server_ctx::commands::command_end;
use yak_server_ctx::ctx::ServerCommandContextTrait;

use crate::ctx::ServerCommandContext;

pub(crate) async fn trace_io_command(
    context: &ServerCommandContext<'_>,
    req: yak_cli_proto::TraceIoRequest,
) -> yak_error::Result<yak_cli_proto::TraceIoResponse> {
    let start_event = context
        .command_start_event(yak_data::TraceIoCommandStart {}.into())
        .await?;
    span_async(start_event, async move {
        let tracing_provider = TracingIoProvider::from_io(&*context.base_context.repo.io);
        let respond_with_trace = matches!(
            req.read_state,
            Some(trace_io_request::ReadIoTracingState { with_trace: true })
        );

        let result = match (tracing_provider, respond_with_trace) {
            (Some(provider), true) => build_response_with_trace(context, provider).await,
            (Some(_), false) => Ok(yak_cli_proto::TraceIoResponse {
                enabled: true,
                trace: Vec::new(),
                external_entries: Vec::new(),
                relative_symlinks: Vec::new(),
                external_symlinks: Vec::new(),
            }),
            (None, _) => Ok(yak_cli_proto::TraceIoResponse {
                enabled: false,
                trace: Vec::new(),
                external_entries: Vec::new(),
                relative_symlinks: Vec::new(),
                external_symlinks: Vec::new(),
            }),
        };

        let end_event = command_end(&result, yak_data::TraceIoCommandEnd {});
        (result, end_event)
    })
    .await
}

async fn build_response_with_trace(
    context: &ServerCommandContext<'_>,
    provider: &TracingIoProvider,
) -> yak_error::Result<yak_cli_proto::TraceIoResponse> {
    // Materialize yak-out paths so they can be archived.
    let buck_out_entries: Vec<_> = provider.trace().buck_out_entries();
    context
        .materializer()
        .ensure_materialized(
            buck_out_entries.clone(),
            MaterializationPurpose::IntermediateOnly,
        )
        .await
        .buck_error_context("Error materializing yak-out paths for trace")?;

    let mut entries = provider.trace().project_entries();
    entries.extend(buck_out_entries);

    let mut relative_symlinks = Vec::new();
    let mut external_symlinks = Vec::new();
    for link in provider.trace().symlinks.iter() {
        match &link.to {
            RawSymlink::Relative(to, _) => {
                relative_symlinks.push(trace_io_response::RelativeSymlink {
                    link: link.at.to_string(),
                    target: to.to_string(),
                });
            }
            RawSymlink::External(external) => {
                external_symlinks.push(trace_io_response::ExternalSymlink {
                    link: link.at.to_string(),
                    target: external.target_str().to_owned(),
                    remaining_path: if external.remaining_path().is_empty() {
                        None
                    } else {
                        Some(external.remaining_path().to_string())
                    },
                });
            }
        }
    }

    let mut trace: Vec<String> = entries.into_iter().map(|path| path.to_string()).collect();
    trace.sort_unstable();

    let mut external_entries: Vec<String> = provider
        .trace()
        .external_entries()
        .into_iter()
        .map(|path| path.to_string())
        .collect();
    external_entries.sort_unstable();

    relative_symlinks.sort_unstable_by(|a, b| a.link.cmp(&b.link));
    external_symlinks.sort_unstable_by(|a, b| a.link.cmp(&b.link));

    Ok(yak_cli_proto::TraceIoResponse {
        enabled: true,
        trace,
        external_entries,
        relative_symlinks,
        external_symlinks,
    })
}
