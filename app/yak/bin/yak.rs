/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(used_with_arg)]

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dupe::Dupe;
use superconsole::Stdin;
use yak::exec;
use yak::process_context::ClientRuntime;
use yak::process_context::ProcessContext;
use yak::process_context::SharedProcessContext;
use yak::soft_error;
use yak_build_info::BUCK2_BUILD_INFO;
use yak_build_info::Buck2BuildInfo;
use yak_client_ctx::events_ctx::EventsCtx;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::restarter::Restarter;
use yak_client_ctx::stdio;
use yak_client_ctx::subscribers::recorder::InvocationRecorder;
use yak_core::logging::LogConfigurationReloadHandle;
use yak_core::logging::init_tracing_for_writer;
use yak_core::logging::log_file::TracingLogFile;
use yak_core::yak_env;
use yak_fs::working_dir::AbsWorkingDir;
use yak_wrapper_common::invocation_id::TraceId;

// Cargo builds use jemalloc on Linux and macOS. A yak build (`cfg(buck_build)`) uses the system
// allocator, because `third-party/rust/fixups/tikv-jemalloc-sys` does not build jemalloc.
#[global_allocator]
#[cfg(all(any(target_os = "linux", target_os = "macos"), not(buck_build)))]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
#[global_allocator]
#[cfg(target_os = "windows")]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn init_logging() -> yak_error::Result<Arc<dyn LogConfigurationReloadHandle>> {
    static ENV_TRACING_LOG_FILE_PATH: &str = "YAK_LOG_TO_FILE_PATH";

    let handle = match std::env::var_os(ENV_TRACING_LOG_FILE_PATH) {
        Some(path) => {
            let path = PathBuf::from(path);
            // we set the writer to stderr first until later, when we have the logdir, set the
            // tracing log sink to that file

            fs::create_dir_all(&path)?;
            let tracing_log = path.join("tracing_log");
            let file = TracingLogFile::new(tracing_log)?;
            init_tracing_for_writer(file)
        }
        _ => init_tracing_for_writer(io::stderr),
    }?;

    Ok(handle)
}

fn print_retry() -> yak_error::Result<()> {
    yak_client_ctx::eprintln!("============================================================")?;
    yak_client_ctx::eprintln!("|| yak has detected that it needs to restart to proceed ||")?;
    yak_client_ctx::eprintln!("|| Your command will now restart.                         ||")?;
    yak_client_ctx::eprintln!("============================================================")?;
    yak_client_ctx::eprintln!()?;
    Ok(())
}

fn exec_with_logging(
    trace_id: TraceId,
    start_time: SystemTime,
    restarted_trace_id: Option<TraceId>,
    shared: yak_error::Result<SharedProcessContext>,
    runtime: &mut ClientRuntime,
) -> (Option<SharedProcessContext>, ExitResult) {
    let args = std::env::args().collect::<Vec<String>>();
    let recorder = InvocationRecorder::new(trace_id.dupe(), restarted_trace_id, start_time, args);
    let mut events_ctx = EventsCtx::new(Some(recorder), vec![]);
    let (shared, res) = match shared {
        Ok(mut shared) => {
            let res = exec(ProcessContext {
                trace_id: trace_id.dupe(),
                events_ctx: &mut events_ctx,
                shared: &mut shared,
                runtime,
                start_time,
            });
            (Some(shared), res)
        }
        Err(e) => (None, e.into()),
    };
    let res = match runtime.get_or_init() {
        Ok(runtime) => events_ctx.finalize_events(trace_id, res, runtime),
        Err(e) => e.into(),
    };
    (shared, res)
}

// As this main() is used as the entry point for the `yak daemon` command,
// it must be single-threaded. Commands that want to be multi-threaded/async
// will start up their own tokio runtime.
fn main() -> ! {
    yak_core::client_only::CLIENT_ONLY_VAL.init(cfg!(client_only));
    #[cfg(not(client_only))]
    {
        yak_analysis::init_late_bindings();
        yak_anon_target::init_late_bindings();
        yak_action_impl::init_late_bindings();
        yak_cmd_audit_server::init_late_bindings();
        yak_build_api::init_late_bindings();
        yak_cmd_docs_server::init_late_bindings();
        yak_external_cells::init_late_bindings();
        yak_transition::init_late_bindings();
        yak_build_signals_impl::init_late_bindings();
        yak_bxl::init_late_bindings();
        yak_cfg_constructor::init_late_bindings();
        yak_configured::init_late_bindings();
        yak_query_impls::init_late_bindings();
        yak_interpreter_for_build::init_late_bindings();
        yak_server_commands::init_late_bindings();
        yak_cmd_targets_server::init_late_bindings();
        yak_cmd_query_server::init_late_bindings();
        yak_cmd_starlark_server::init_late_bindings();
        yak_test::init_late_bindings();
        yak_validation::init_late_bindings();
        yak_events::init_late_bindings();
    }
    BUCK2_BUILD_INFO.init(Buck2BuildInfo {
        revision: std::option_env!("YAK_SET_EXPLICIT_VERSION"),
    });

    // Set up crypto impl once per process
    yak_certs::certs::setup_cryptography_or_fail();

    fn init_shared_context() -> yak_error::Result<SharedProcessContext> {
        soft_error::initialize()?;

        // Log the start timestamp
        tracing::debug!("Client initialized logging");

        let stdin_buffer_size = yak_env!(
            "YAK_TEST_STDIN_BUFFER_SIZE",
            type=usize,
            applicability=testing,
        )?
        .unwrap_or(8192);

        Ok(SharedProcessContext {
            log_reload_handle: init_logging()?,
            stdin: Stdin::new(stdin_buffer_size),
            working_dir: AbsWorkingDir::current_dir()?,
            args: std::env::args().collect::<Vec<String>>(),
            restarter: Restarter::new(),
            force_want_restart: yak_env!("FORCE_WANT_RESTART", bool)?,
        })
    }

    fn main_with_result() -> ExitResult {
        let start_time = SystemTime::now();
        let first_trace_id = TraceId::from_env_or_new()?;
        let mut runtime = ClientRuntime::new();
        let shared = init_shared_context();
        let (shared, res) = exec_with_logging(
            first_trace_id.dupe(),
            start_time,
            None,
            shared,
            &mut runtime,
        );

        if let Some(shared) = shared {
            maybe_restart(first_trace_id, res, shared, &mut runtime)
        } else {
            res
        }
    }

    fn maybe_restart(
        first_trace_id: TraceId,
        initial_result: ExitResult,
        shared: SharedProcessContext,
        runtime: &mut ClientRuntime,
    ) -> ExitResult {
        let force_want_restart = shared.force_want_restart;
        let restart = |res| {
            let restart_start_time = SystemTime::now();

            if !force_want_restart && !shared.restarter.should_restart() {
                tracing::debug!("No restart was requested");
                return res;
            }

            if stdio::has_written_to_stdout() {
                tracing::debug!("Cannot restart: wrote to stdout");
                return res;
            }

            if print_retry().is_err() {
                tracing::debug!("Cannot restart: warning message cannot be printed");
                return res;
            }

            let (_, res) = exec_with_logging(
                TraceId::new(),
                restart_start_time,
                Some(first_trace_id),
                Ok(shared),
                runtime,
            );
            res
        };

        if force_want_restart {
            restart(initial_result)
        } else {
            initial_result.or_else(restart)
        }
    }

    main_with_result().report()
}
