/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use buck2_error::BuckErrorContext;
use bytes::Bytes;
use dupe::Dupe;
use futures::StreamExt;
use futures::TryStreamExt;
use futures::stream::BoxStream;
use http::Method;
use http::request::Builder;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper_util::client::legacy::ResponseFuture;
use hyper_util::client::legacy::connect::Connect;
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use tokio_util::io::StreamReader;

use crate::HttpError;
use crate::redirect::PendingRequest;
use crate::redirect::RedirectEngine;
use crate::stats::CountingStream;
use crate::stats::HttpNetworkStats;

mod builder;
pub use builder::HttpClientBuilder;

const DEFAULT_USER_AGENT: &str = "Buck2";

#[derive(Allocative, Clone, Dupe)]
pub struct HttpClient {
    // hyper::Client doesn't impl Allocative.
    #[allocative(skip)]
    inner: Arc<dyn RequestClient>,
    max_redirects: Option<usize>,
    http2: bool,
    stats: HttpNetworkStats,
    // tokio::sync::Semaphore doesn't impl Allocative
    #[allocative(skip)]
    concurrent_requests_budget: Option<Arc<Semaphore>>,
}

impl HttpClient {
    fn request_builder(&self, uri: &str) -> Builder {
        Request::builder()
            .uri(uri)
            .header(http::header::USER_AGENT, DEFAULT_USER_AGENT)
    }

    /// Send a HEAD request. Assumes no body will be returned. If one is returned, it will be ignored.
    pub async fn head(&self, uri: &str) -> Result<Response<()>, HttpError> {
        let req = self
            .request_builder(uri)
            .method(Method::HEAD)
            .body(Bytes::new())
            .map_err(HttpError::BuildRequest)?;
        self.request(req).await.map(|resp| resp.map(|_| ()))
    }

    /// Send a GET request.
    pub async fn get(
        &self,
        uri: &str,
    ) -> Result<Response<BoxStream<'_, hyper::Result<Bytes>>>, HttpError> {
        let req = self
            .request_builder(uri)
            .method(Method::GET)
            .body(Bytes::new())
            .map_err(HttpError::BuildRequest)?;
        self.request(req).await
    }

    pub async fn post(
        &self,
        uri: &str,
        body: Bytes,
        headers: Vec<(String, String)>,
    ) -> Result<Response<BoxStream<'_, hyper::Result<Bytes>>>, HttpError> {
        let mut builder = self.request_builder(uri).method(Method::POST);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let req = builder.body(body).map_err(HttpError::BuildRequest)?;
        self.request(req).await
    }

    pub async fn put(
        &self,
        uri: &str,
        body: Bytes,
        headers: Vec<(String, String)>,
    ) -> Result<Response<BoxStream<'_, hyper::Result<Bytes>>>, HttpError> {
        let mut builder = self.request_builder(uri).method(Method::PUT);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let req = builder.body(body).map_err(HttpError::BuildRequest)?;
        self.request(req).await
    }

    async fn send_request_impl(
        &self,
        request: Request<Bytes>,
    ) -> Result<Response<BoxStream<'_, hyper::Result<Bytes>>>, HttpError> {
        let uri = request.uri().to_string();
        let now = tokio::time::Instant::now();

        let semaphore_guard = match self.concurrent_requests_budget.as_ref() {
            Some(sem) => Some(
                sem.acquire()
                    .await
                    .expect("Semaphore should never be closed"),
            ),
            None => None,
        };

        let resp = self.inner.request(request).await.map_err(|e| {
            if is_hyper_error_due_to_timeout(&e) {
                HttpError::Timeout {
                    uri,
                    duration: (tokio::time::Instant::now() - now).as_secs(),
                }
            } else {
                HttpError::SendRequest { uri, source: e }
            }
        })?;
        Ok(resp.map(move |body| {
            CountingStream::new(
                body.into_data_stream(),
                self.stats.downloaded_bytes().dupe(),
            )
            .inspect(move |_| {
                // Ensure we keep a concurrent request permit alive until the stream is consumed
                let _guard = &semaphore_guard;
            })
            .boxed()
        }))
    }

    /// Send a generic request.
    pub async fn request(
        &self,
        request: Request<Bytes>,
    ) -> Result<Response<BoxStream<'_, hyper::Result<Bytes>>>, HttpError> {
        let pending_request = PendingRequest::from_request(&request);
        let uri = request.uri().clone();
        tracing::debug!("http: request: {:?}", request);
        let resp = self.send_request_impl(request).await?;
        tracing::debug!("http: response: {:?}", resp.status());

        // Handle redirects up to self.max_redirects times.
        let resp = if let Some(max_redirects) = self.max_redirects {
            let redirect_engine = RedirectEngine::new(max_redirects, pending_request, resp);
            redirect_engine
                .handle_redirects(|req| self.send_request_impl(req))
                .await?
        } else {
            resp
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = read_truncated_error_response(resp).await;
            return Err(HttpError::Status {
                status,
                uri: uri.to_string(),
                text,
            });
        }

        Ok(resp)
    }

    pub fn stats(&self) -> &HttpNetworkStats {
        &self.stats
    }

    pub fn http2(&self) -> bool {
        self.http2
    }
}

/// Trait wrapper around a hyper::Client because hyper::Client is parameterized by
/// the connector. At runtime, we want to pick different connectors (e.g. HttpsConnector,
/// ProxyConnector<HttpsConnector<..>>, etc); thus wrap the client so we can switch
/// out the concrete type without exposing implementation details to callers.
pub(super) trait RequestClient: Send + Sync {
    fn request(&self, request: Request<Bytes>) -> ResponseFuture;
}

impl<C> RequestClient for hyper_util::client::legacy::Client<C, Full<Bytes>>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    fn request(&self, request: Request<Bytes>) -> ResponseFuture {
        let mapped_request: Request<Full<Bytes>> = request.map(Full::new);
        self.request(mapped_request)
    }
}

async fn read_truncated_error_response(
    mut resp: Response<BoxStream<'_, hyper::Result<Bytes>>>,
) -> String {
    let read = StreamReader::new(resp.body_mut().map_err(std::io::Error::other));
    let mut buf = Vec::with_capacity(1024);
    read.take(1024).read_to_end(&mut buf).await.map_or_else(
        |e| format!("Error decoding response: {e:?}"),
        |_| String::from_utf8_lossy(buf.as_ref()).into_owned(),
    )
}

/// Helper function to consume a response stream and convert it to a Bytes container.
/// Warning: This does no length checking (like hyper::body::to_bytes). Should
/// only be used for trusted endpoints.
pub async fn to_bytes(body: BoxStream<'_, hyper::Result<Bytes>>) -> buck2_error::Result<Bytes> {
    let mut reader = StreamReader::new(body.map_err(std::io::Error::other));
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .await
        .buck_error_context("Reading response body")?;
    Ok(buf.into())
}

/// Helper function to check if any error in the chain of errors produced by
/// hyper is due to a timeout.
fn is_hyper_error_due_to_timeout(e: &hyper_util::client::legacy::Error) -> bool {
    use std::error::Error;

    let mut cause = e.source();
    while let Some(err) = cause {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            if let std::io::ErrorKind::TimedOut = io_err.kind() {
                return true;
            }
        }
        cause = err.source();
    }

    false
}

#[cfg(test)]
mod tests {
    use http::StatusCode;
    use httptest::Expectation;
    use httptest::matchers::*;
    use httptest::responders;

    use super::*;

    #[tokio::test]
    async fn test_simple_get_success() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots().await?.build();
        let resp = client.get(&test_server.url_str("/foo")).await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_simple_put_success() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(all_of![
                request::method_path("PUT", "/foo"),
                request::body("Hello, world!")
            ])
            .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots().await?.build();
        let bytes = Bytes::from_static(b"Hello, world!");
        let resp = client
            .put(
                &test_server.url_str("/foo"),
                bytes,
                vec![("key".to_owned(), "value".to_owned())],
            )
            .await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_simple_post_success() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(all_of![
                request::method_path("POST", "/foo"),
                request::body("Hello, world!")
            ])
            .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots().await?.build();
        let bytes = Bytes::from_static(b"Hello, world!");
        let resp = client
            .post(
                &test_server.url_str("/foo"),
                bytes,
                vec![("key".to_owned(), "value".to_owned())],
            )
            .await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_404_not_found_is_error() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .respond_with(responders::status_code(404)),
        );

        let client = HttpClientBuilder::https_with_system_roots().await?.build();
        let url = test_server.url_str("/foo");
        let result = client.get(&url).await;
        assert!(result.is_err());
        if let HttpError::Status { status, uri, text } = result.as_ref().err().unwrap() {
            assert_eq!(StatusCode::NOT_FOUND, *status);
            assert_eq!(url.to_owned(), *uri);
            assert!(text.is_empty());
        } else {
            unreachable!(
                "Expected HttpError::Status, got {:?}",
                result.err().unwrap()
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_count_response_size() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .times(2)
                // Response body is 100 bytes in size.
                .respond_with(responders::status_code(200).body(vec![0; 100])),
        );

        let client = HttpClientBuilder::https_with_system_roots().await?.build();
        let mut resp = client.get(&test_server.url_str("/foo")).await?;

        // Consume the stream so we trigger a count.
        while (resp.body_mut().next().await).is_some() {}
        assert_eq!(100, client.stats().get_downloaded_bytes());

        let mut resp = client.get(&test_server.url_str("/foo")).await?;

        // Consume the stream so we trigger a count.
        while (resp.body_mut().next().await).is_some() {}
        assert_eq!(200, client.stats().get_downloaded_bytes());

        Ok(())
    }

    #[tokio::test]
    async fn test_follows_redirects() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        // Chain of two redirects /foo -> /bar -> /baz.
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/bar"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/bar"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/baz"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/baz"))
                .times(1)
                .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(10)
            .build();
        let resp = client.get(&test_server.url_str("/foo")).await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_head_changes_to_get_on_redirect() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        // Chain of two redirects /foo -> /bar -> /baz.
        test_server.expect(
            Expectation::matching(request::method_path("HEAD", "/foo"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/bar"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/bar"))
                .times(1)
                .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(10)
            .build();
        let resp = client.head(&test_server.url_str("/foo")).await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_post_gets_redirected() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        // Redirect /foo -> /bar
        test_server.expect(
            Expectation::matching(all_of![
                request::method_path("POST", "/foo"),
                request::body("Hello, world!"),
            ])
            .times(1)
            .respond_with(
                responders::status_code(307).append_header(http::header::LOCATION, "/bar"),
            ),
        );
        test_server.expect(
            Expectation::matching(all_of![
                request::method_path("POST", "/bar"),
                request::body("Hello, world!"),
                request::headers(not(contains(key(hyper::header::ORIGIN.as_str())))),
                request::headers(not(contains(key(hyper::header::AUTHORIZATION.as_str())))),
                request::headers(not(contains(key(hyper::header::WWW_AUTHENTICATE.as_str())))),
                request::headers(not(contains(key(hyper::header::COOKIE.as_str())))),
                request::headers(not(contains(key(
                    hyper::header::PROXY_AUTHORIZATION.as_str()
                )))),
            ])
            .times(1)
            .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(10)
            .build();
        let bytes = Bytes::from_static(b"Hello, world!");
        let resp = client
            .post(
                &test_server.url_str("/foo"),
                bytes,
                vec![("key".to_owned(), "value".to_owned())],
            )
            .await?;
        assert_eq!(200, resp.status().as_u16());

        Ok(())
    }

    #[tokio::test]
    async fn test_too_many_redirects_fails() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        // Chain of three redirects /foo -> /bar -> /baz -> /boo.
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/bar"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/bar"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/baz"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/baz"))
                .times(1)
                .respond_with(
                    responders::status_code(302).append_header(http::header::LOCATION, "/boo"),
                ),
        );
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/boo"))
                .times(0)
                .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(1)
            .build();
        let url = test_server.url_str("/foo");
        let result = client.get(&url).await;
        if let HttpError::TooManyRedirects { uri, max_redirects } = result.as_ref().err().unwrap() {
            assert_eq!(url.to_owned(), *uri);
            assert_eq!(1, *max_redirects);
        } else {
            unreachable!(
                "Expected HttpError::TooManyRedirects, got {:?}",
                result.err().unwrap()
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrency_limit() -> buck2_error::Result<()> {
        buck2_certs::certs::maybe_setup_cryptography();
        let test_server = httptest::Server::run();
        test_server.expect(
            Expectation::matching(request::method_path("GET", "/foo"))
                .times(3)
                .respond_with(responders::status_code(200)),
        );

        let client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_concurrent_requests(Some(2))
            .build();
        let url = test_server.url_str("/foo");
        let req1 = client.get(&url).await?;
        let req2 = client.get(&url).await?;
        assert_eq!(
            client
                .concurrent_requests_budget
                .as_ref()
                .unwrap()
                .available_permits(),
            0
        );
        let mut req3 = std::pin::pin!(client.get(&url));
        // TODO: Use `tokio::time::pause` to make this faster and deterministic. Blocked by
        // https://github.com/ggriffiniii/httptest/issues/29.
        assert!(
            tokio::time::timeout(tokio::time::Duration::from_millis(100), &mut req3)
                .await
                .is_err()
        );
        drop(req1);
        req3.await?;
        assert_eq!(
            client
                .concurrent_requests_budget
                .as_ref()
                .unwrap()
                .available_permits(),
            1
        );
        drop(req2);
        assert_eq!(
            client
                .concurrent_requests_budget
                .as_ref()
                .unwrap()
                .available_permits(),
            2
        );

        Ok(())
    }
}

// TODO(skarlage): Debug why these tests fail on CircleCI
