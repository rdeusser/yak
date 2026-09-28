/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Routes soft errors to the event stream of the command that raised them. The buck2 CLI and
//! daemon share this handler.

use std::sync::Arc;

use buck2_core::error::SoftErrorContext;
use buck2_core::error::StructuredErrorOptions;
use buck2_data::Location;
use buck2_error::BuckErrorContext;

/// Installs the soft error handler.
pub fn initialize() -> buck2_error::Result<()> {
    buck2_core::error::initialize(
        Box::new(move |category, err, loc, context, options| {
            write_soft_error(
                category,
                err,
                Location {
                    file: loc.0.to_owned(),
                    line: loc.1,
                    column: loc.2,
                },
                context,
                options,
            );
        }),
        Box::new(|| {
            buck2_events::dispatch::get_dispatcher_opt()
                .and_then(|dispatcher| dispatcher.soft_error_context())
        }),
    )
    .buck_error_context("Error initializing soft errors")?;
    Ok(())
}

fn write_soft_error(
    category: &str,
    err: &buck2_error::Error,
    location: Location,
    context: &Arc<SoftErrorContext>,
    options: StructuredErrorOptions,
) {
    let event = buck2_data::StructuredError {
        location: Some(location),
        payload: format!("Soft Error: {category}: {err:#}"),
        quiet: options.quiet,
        soft_error_category: Some(buck2_data::SoftError {
            category: category.to_owned(),
            is_quiet: options.quiet,
        }),
        daemon_in_memory_state_is_corrupted: options.daemon_in_memory_state_is_corrupted,
        daemon_materializer_state_is_corrupted: options.daemon_materializer_state_is_corrupted,
    };

    // If the soft error was fired in a context with an ambient dispatcher, then we only send
    // it there, but some contexts don't have one, and in that case, we notify all running
    // commands.
    match buck2_events::dispatch::get_dispatcher_opt() {
        Some(dispatcher) => {
            dispatcher.instant_event(event);
        }
        None => {
            #[cfg(client_only)]
            let warn = !options.quiet;
            #[cfg(not(client_only))]
            let warn = {
                let sent = if context.is_command_scoped() {
                    buck2_server::active_commands::dispatch_soft_error_for_context(context, &event)
                } else {
                    buck2_server::active_commands::broadcast_instant_event(&event)
                };
                !sent && !options.quiet
            };
            if warn {
                tracing::warn!("Warning \"{}\": {:#}", category, err);
            }
        }
    }
}
