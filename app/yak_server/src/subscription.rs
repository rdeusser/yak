/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::time::Duration;

use futures::future::FutureExt;
use tokio::time::MissedTickBehavior;
use yak_error::YakErrorOptionContext;
use yak_error::ErrorTag;
use yak_error::yak_error;
use yak_events::dispatch::span_async;
use yak_server_ctx::commands::command_end;
use yak_server_ctx::ctx::ServerCommandContextTrait;
use yak_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use yak_server_ctx::streaming_request_handler::StreamingRequestHandler;

use crate::active_commands;

pub(crate) async fn run_subscription_server_command(
    ctx: &dyn ServerCommandContextTrait,
    mut partial_result_dispatcher: PartialResultDispatcher<
        yak_cli_proto::SubscriptionResponseWrapper,
    >,
    mut req: StreamingRequestHandler<yak_cli_proto::SubscriptionRequestWrapper>,
) -> yak_error::Result<yak_cli_proto::SubscriptionCommandResponse> {
    let start_event = ctx
        .command_start_event(yak_data::SubscriptionCommandStart {}.into())
        .await?;
    span_async(start_event, async move {
        let result: yak_error::Result<yak_cli_proto::SubscriptionCommandResponse> = try {
            let mut wants_active_commands = false;

            let mut ticker = tokio::time::interval(Duration::from_millis(100));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let disconnect = loop {
                futures::select! {
                    message = req.message().fuse() => {
                        use yak_subscription_proto::subscription_request::Request;

                        let message = message?.request.internal_error("Empty subscription message");
                        let request = message?.request.ok_or_else(|| {
                            yak_error!(
                                ErrorTag::SubscriptionEmptyRequest,
                                "Empty subscription request"
                            )
                        })?;
                        match request {
                            Request::Disconnect(disconnect) => {
                                break disconnect;
                            }
                            Request::SubscribeToActiveCommands(yak_subscription_proto::SubscribeToActiveCommands {}) => {
                                wants_active_commands = true;
                            }
                        }
                    }
                    _ = ticker.tick().fuse() => {
                        if wants_active_commands {
                            let snapshot = yak_subscription_proto::ActiveCommandsSnapshot {
                                active_commands: active_commands::active_commands_snapshot(),
                            };
                            partial_result_dispatcher.emit(yak_cli_proto::SubscriptionResponseWrapper {
                                response: Some(yak_subscription_proto::SubscriptionResponse {
                                    response: Some(snapshot.into())
                                })
                            });
                        }
                    }
                }
            };

            partial_result_dispatcher.emit(yak_cli_proto::SubscriptionResponseWrapper {
                response: Some(yak_subscription_proto::SubscriptionResponse {
                    response: Some(yak_subscription_proto::Goodbye {
                        reason: disconnect.reason,
                        ok: disconnect.ok,
                    }.into())
                })
            });

            yak_cli_proto::SubscriptionCommandResponse {}
        };

        let end_event = command_end(&result, yak_data::SubscriptionCommandEnd {});
        (result, end_event)
    })
    .await
}
