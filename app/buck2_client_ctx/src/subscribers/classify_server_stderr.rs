/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::ops::ControlFlow;
use std::sync::LazyLock;

use buck2_data::error::ErrorTag;

pub(crate) fn classify_server_stderr(
    error: buck2_error::Error,
    stderr: &str,
) -> buck2_error::Error {
    let mut tag = if stderr.is_empty() {
        None
    } else if stderr.contains("<jemalloc>: size mismatch detected") {
        Some(ErrorTag::ServerJemallocAssert)
    } else if stderr.contains("panicked at") {
        Some(ErrorTag::ServerPanicked)
    } else if stderr.contains("has overflowed its stack") {
        // Stderr looks like this:
        // ```
        // thread 'buck2-dm' has overflowed its stack
        // ```
        Some(ErrorTag::ServerStackOverflow)
    } else if stderr.contains("Resource temporarily unavailable") {
        Some(ErrorTag::EAgain)
    } else {
        None
    };
    if tag.is_none() && error.has_tag(ErrorTag::ClientGrpcStream) {
        tag = Some(ErrorTag::DaemonDisconnect);
    }

    let mut tags = tag.into_iter().collect::<Vec<_>>();

    let error = if let Some(trace) = extract_trace(stderr) {
        if trace
            .stack_trace_lines
            .iter()
            .any(|line| line.contains("remote_execution"))
        {
            tags.push(ErrorTag::ReClientCrash);
        }

        error.string_tag(&format!("crash({})", trace.trace_key()))
    } else {
        error
    };

    error.tag(tags)
}

//    0: rust_begin_unwind
//       at /rustc/library/std/src/panicking.rs:652:5
//    1: <buck2_server::daemon::server::BuckdServer as buck2_cli_proto::daemon_api_server::DaemonApi>::unstable_crash::{closure#0}
//       at ./app/buck2_server/src/daemon/crash.rs:18:13
static RUST_STACK_FRAME: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^\s*\d*:\s*(.*)$").unwrap());
static RUST_CONTEXT: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^\s*at \S*$").unwrap());

fn extract_rust_frame(line: &str) -> ControlFlow<(), Option<String>> {
    if let Some(capture) = RUST_STACK_FRAME
        .captures(line)
        .and_then(|captures| captures.get(1))
    {
        ControlFlow::Continue(Some(capture.as_str().to_owned()))
    } else if RUST_CONTEXT.is_match(line) {
        ControlFlow::Continue(None)
    } else {
        ControlFlow::Break(())
    }
}

const UNINTERESTING_SEGMENTS: [&str; 2] = ["rust_begin_unwind", "core::panicking::panic_fmt"];

struct StackTraceInfo {
    stack_trace_lines: Vec<String>,
}

impl StackTraceInfo {
    fn sanitized_trace(self) -> Vec<String> {
        // Exclude some lines to reduce churn in trace keys.
        // Changes higher up the stack are less likely to be related to the crash.
        if self.stack_trace_lines.len() > 5 {
            // 'uninteresting' lines shouldn't cause churn but exclude them if we are shortening the stack
            // to get enough unique data.

            self.stack_trace_lines
                .into_iter()
                .filter(|line| !UNINTERESTING_SEGMENTS.iter().any(|s| line.contains(s)))
                .take(5)
                .collect()
        } else {
            self.stack_trace_lines.clone()
        }
    }

    fn trace_key(self) -> String {
        // blake3 just because the default rust hasher isn't intended to be stable.
        let mut hasher = blake3::Hasher::new();
        for s in self.sanitized_trace() {
            hasher.update(s.as_bytes());
        }
        let mut digest = hasher.finalize().to_string();
        // Truncate to keep category_key relatively short and readable.
        // This should be enough to avoid collisions since there should be very few unique crashes.
        digest.truncate(6);
        digest
    }
}

fn extract_trace(stderr: &str) -> Option<StackTraceInfo> {
    let mut lines = stderr.split('\n');
    lines.find(|line| line.starts_with("stack backtrace:"))?;

    let mut stack_trace_lines: Vec<String> = vec![];
    for line in lines {
        match extract_rust_frame(line) {
            ControlFlow::Continue(Some(line)) => stack_trace_lines.push(line),
            ControlFlow::Continue(None) => (),
            ControlFlow::Break(_) => break,
        }
    }

    Some(StackTraceInfo { stack_trace_lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_trace_lines(trace: &str) -> Vec<String> {
        trace
            .split('\n')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn test_generated_stack_trace() {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let stderr = format!("stack backtrace:\n{backtrace}");
        assert!(extract_trace(&stderr).is_some());
    }

    #[test]
    fn test_rust_stack_trace_hash() {
        // from `buck2 debug crash panic`
        let panic_trace = "
stack backtrace:
   0: rust_begin_unwind
             at /rustc/library/std/src/panicking.rs:652:5
   1: core::panicking::panic_fmt
             at /rustc/library/core/src/panicking.rs:72:14
   2: <buck2_server::daemon::server::BuckdServer as buck2_cli_proto::daemon_api_server::DaemonApi>::unstable_crash::{closure#0}
             at ./app/buck2_server/src/daemon/crash.rs:18:13
   3: <<buck2_cli_proto::daemon_api_server::DaemonApiServer<_> as tower_service::Service<http::request::Request<_>>>::call::Unstable_CrashSvc<buck2_server::daemon::server::BuckdServer> as tonic::server::service::UnaryService<buck2_cli_proto::UnstableCrashRequest>>::call::{closure#0}
             at /rustc/library/core/src/future/future.rs:123:9
   4: <buck2_cli_proto::daemon_api_server::DaemonApiServer<buck2_server::daemon::server::BuckdServer> as tower_service::Service<http::request::Request<hyper::body::body::Body>>>::call::{closure#20}
             at /rustc/library/core/src/future/future.rs:123:9
   5: <futures_util::future::future::map::Map<futures_util::future::try_future::into_future::IntoFuture<core::pin::Pin<alloc::boxed::Box<dyn core::future::future::Future<Output = core::result::Result<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, tonic::status::Status>>, core::convert::Infallible>> + core::marker::Send>>>, futures_util::fns::MapOkFn<<tonic::transport::service::router::Routes>::add_service<buck2_test_proto::test_executor_server::TestExecutorServer<buck2_test_api::grpc::executor::Service<buck2_test_runner::executor::Buck2TestExecutor>>>::{closure#0}>> as core::future::future::Future>::poll
             at /rustc/library/core/src/future/future.rs:123:9
   6: <futures_util::future::future::map::Map<futures_util::future::try_future::into_future::IntoFuture<tower::util::map_response::MapResponseFuture<core::pin::Pin<alloc::boxed::Box<dyn core::future::future::Future<Output = core::result::Result<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, tonic::status::Status>>, core::convert::Infallible>> + core::marker::Send>>, <tonic::transport::service::router::Routes>::add_service<buck2_forkserver_proto::forkserver_server::ForkserverServer<buck2_forkserver::unix::service::UnixForkserverService>>::{closure#0}>>, futures_util::fns::MapOkFn<<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, axum_core::error::Error>> as axum_core::response::into_response::IntoResponse>::into_response>> as core::future::future::Future>::poll
             at ./vendor/futures-util-0.3.30/src/lib.rs:91:13
   7: <tower::util::map_response::MapResponseFuture<tower::util::map_response::MapResponseFuture<core::pin::Pin<alloc::boxed::Box<dyn core::future::future::Future<Output = core::result::Result<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, tonic::status::Status>>, core::convert::Infallible>> + core::marker::Send>>, <tonic::transport::service::router::Routes>::add_service<buck2_cli_proto::daemon_api_server::DaemonApiServer<buck2_server::daemon::server::BuckdServer>>::{closure#0}>, <http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, axum_core::error::Error>> as axum_core::response::into_response::IntoResponse>::into_response> as core::future::future::Future>::poll
             at ./vendor/futures-util-0.3.30/src/lib.rs:91:13
   8: <tower::util::oneshot::Oneshot<tower::util::boxed_clone::BoxCloneService<http::request::Request<hyper::body::body::Body>, http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, axum_core::error::Error>>, core::convert::Infallible>, http::request::Request<hyper::body::body::Body>> as core::future::future::Future>::poll
             at /rustc/library/core/src/future/future.rs:123:9
             ";

        let panic_sanitized_trace = test_trace_lines("
        <buck2_server::daemon::server::BuckdServer as buck2_cli_proto::daemon_api_server::DaemonApi>::unstable_crash::{closure#0}
        <<buck2_cli_proto::daemon_api_server::DaemonApiServer<_> as tower_service::Service<http::request::Request<_>>>::call::Unstable_CrashSvc<buck2_server::daemon::server::BuckdServer> as tonic::server::service::UnaryService<buck2_cli_proto::UnstableCrashRequest>>::call::{closure#0}
        <buck2_cli_proto::daemon_api_server::DaemonApiServer<buck2_server::daemon::server::BuckdServer> as tower_service::Service<http::request::Request<hyper::body::body::Body>>>::call::{closure#20}
        <futures_util::future::future::map::Map<futures_util::future::try_future::into_future::IntoFuture<core::pin::Pin<alloc::boxed::Box<dyn core::future::future::Future<Output = core::result::Result<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, tonic::status::Status>>, core::convert::Infallible>> + core::marker::Send>>>, futures_util::fns::MapOkFn<<tonic::transport::service::router::Routes>::add_service<buck2_test_proto::test_executor_server::TestExecutorServer<buck2_test_api::grpc::executor::Service<buck2_test_runner::executor::Buck2TestExecutor>>>::{closure#0}>> as core::future::future::Future>::poll
        <futures_util::future::future::map::Map<futures_util::future::try_future::into_future::IntoFuture<tower::util::map_response::MapResponseFuture<core::pin::Pin<alloc::boxed::Box<dyn core::future::future::Future<Output = core::result::Result<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, tonic::status::Status>>, core::convert::Infallible>> + core::marker::Send>>, <tonic::transport::service::router::Routes>::add_service<buck2_forkserver_proto::forkserver_server::ForkserverServer<buck2_forkserver::unix::service::UnixForkserverService>>::{closure#0}>>, futures_util::fns::MapOkFn<<http::response::Response<http_body::combinators::box_body::UnsyncBoxBody<bytes::bytes::Bytes, axum_core::error::Error>> as axum_core::response::into_response::IntoResponse>::into_response>> as core::future::future::Future>::poll
        ");
        assert_eq!(
            extract_trace(panic_trace)
                .expect("no trace found")
                .sanitized_trace(),
            panic_sanitized_trace
        );
    }
}
