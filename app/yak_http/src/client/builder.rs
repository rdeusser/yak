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
use std::time::Duration;

use hyper::Uri;
use hyper_http_proxy::Proxy;
use hyper_http_proxy::ProxyConnector;
use hyper_rustls::HttpsConnector;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_timeout::TimeoutConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use rustls::ClientConfig;
use tokio::sync::Semaphore;
use tokio_rustls::TlsConnector;
use tower_service::Service as TowerService;
use yak_certs::certs::tls_config_with_system_roots;

use super::HttpClient;
use super::RequestClient;
use crate::proxy;
use crate::stats::HttpNetworkStats;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimeoutConfig {
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl TimeoutConfig {
    fn to_connector<C>(&self, connector: C) -> TimeoutConnector<C>
    where
        C: TowerService<Uri> + Send,
        C::Response: hyper::rt::Read + hyper::rt::Write + Send + Unpin,
        C::Future: Send + 'static,
        C::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let mut timeout_connector = TimeoutConnector::new(connector);
        timeout_connector.set_connect_timeout(self.connect_timeout);
        timeout_connector.set_read_timeout(self.read_timeout);
        timeout_connector.set_write_timeout(self.write_timeout);
        timeout_connector
    }
}

pub struct HttpClientBuilder {
    tls_config: ClientConfig,
    proxies: Vec<Proxy>,
    max_redirects: Option<usize>,
    http2: bool,
    timeout_config: Option<TimeoutConfig>,
    max_concurrent_requests: Option<usize>,
}

impl HttpClientBuilder {
    /// `oss` builds an https client that trusts the system roots and uses the proxies named by
    /// `HTTPS_PROXY` and `HTTP_PROXY`.
    pub async fn oss() -> yak_error::Result<Self> {
        let mut builder = Self::https_with_system_roots().await?;
        builder.with_proxy_from_env()?;
        Ok(builder)
    }

    /// Creates a barebones https client using system roots for TLS authentication.
    pub async fn https_with_system_roots() -> yak_error::Result<Self> {
        let tls_config = tls_config_with_system_roots().await?;
        Ok(Self {
            tls_config,
            proxies: Vec::new(),
            max_redirects: None,
            http2: true,
            timeout_config: None,
            max_concurrent_requests: None,
        })
    }

    pub fn with_tls_config(&mut self, tls_config: ClientConfig) -> &mut Self {
        self.tls_config = tls_config;
        self
    }

    pub fn with_proxy(&mut self, proxy: Proxy) -> &mut Self {
        self.proxies.push(proxy);
        self
    }

    pub fn with_proxy_from_env(&mut self) -> yak_error::Result<&mut Self> {
        if let Some(proxy) = proxy::https_proxy_from_env()? {
            self.with_proxy(proxy);
        }
        if let Some(proxy) = proxy::http_proxy_from_env()? {
            self.with_proxy(proxy);
        }
        Ok(self)
    }

    pub fn with_connect_timeout(&mut self, connect_timeout: Option<Duration>) -> &mut Self {
        if let Some(timeout_config) = &mut self.timeout_config {
            timeout_config.connect_timeout = connect_timeout;
        } else {
            self.timeout_config = Some(TimeoutConfig {
                connect_timeout,
                read_timeout: None,
                write_timeout: None,
            });
        }
        self
    }

    pub fn connect_timeout(&self) -> Option<Duration> {
        self.timeout_config.as_ref().and_then(|c| c.connect_timeout)
    }

    pub fn with_read_timeout(&mut self, read_timeout: Option<Duration>) -> &mut Self {
        if let Some(timeout_config) = &mut self.timeout_config {
            timeout_config.read_timeout = read_timeout;
        } else {
            self.timeout_config = Some(TimeoutConfig {
                read_timeout,
                connect_timeout: None,
                write_timeout: None,
            });
        }
        self
    }

    pub fn read_timeout(&self) -> Option<Duration> {
        self.timeout_config.as_ref().and_then(|c| c.read_timeout)
    }

    pub fn with_write_timeout(&mut self, write_timeout: Option<Duration>) -> &mut Self {
        if let Some(timeout_config) = &mut self.timeout_config {
            timeout_config.write_timeout = write_timeout;
        } else {
            self.timeout_config = Some(TimeoutConfig {
                write_timeout,
                connect_timeout: None,
                read_timeout: None,
            });
        }
        self
    }

    pub fn write_timeout(&self) -> Option<Duration> {
        self.timeout_config.as_ref().and_then(|c| c.write_timeout)
    }

    pub fn with_max_redirects(&mut self, max_redirects: usize) -> &mut Self {
        self.max_redirects = Some(max_redirects);
        self
    }

    pub fn max_redirects(&self) -> Option<usize> {
        self.max_redirects
    }

    pub fn with_http2(&mut self, http2: bool) -> &mut Self {
        self.http2 = http2;
        self
    }

    pub fn with_max_concurrent_requests(
        &mut self,
        max_concurrent_requests: Option<usize>,
    ) -> &mut Self {
        self.max_concurrent_requests = max_concurrent_requests;
        self
    }

    fn build_inner(&self) -> Arc<dyn RequestClient> {
        match (self.proxies.as_slice(), &self.timeout_config) {
            // Proxied http client with TLS.
            (proxies @ [_, ..], Some(timeout_config)) => {
                let https_connector = build_https_connector(self.tls_config.clone(), self.http2);
                let timeout_connector = timeout_config.to_connector(https_connector);
                // Re-use TLS config from https connection for communication with proxies.
                let proxy_connector =
                    build_proxy_connector(proxies, timeout_connector, self.tls_config.clone());
                Arc::new(Client::builder(TokioExecutor::new()).build(proxy_connector))
            }
            (proxies @ [_, ..], None) => {
                let https_connector = build_https_connector(self.tls_config.clone(), self.http2);
                let proxy_connector =
                    build_proxy_connector(proxies, https_connector, self.tls_config.clone());
                Arc::new(Client::builder(TokioExecutor::new()).build(proxy_connector))
            }

            // Client with TLS only.
            ([], Some(timeout_config)) => {
                let https_connector = build_https_connector(self.tls_config.clone(), self.http2);
                let timeout_connector = timeout_config.to_connector(https_connector);
                Arc::new(Client::builder(TokioExecutor::new()).build(timeout_connector))
            }
            ([], None) => {
                let https_connector = build_https_connector(self.tls_config.clone(), self.http2);
                Arc::new(Client::builder(TokioExecutor::new()).build(https_connector))
            }
        }
    }

    pub fn build(&self) -> HttpClient {
        HttpClient {
            inner: self.build_inner(),
            max_redirects: self.max_redirects,
            http2: self.http2,
            stats: HttpNetworkStats::new(),
            concurrent_requests_budget: self
                .max_concurrent_requests
                .map(|v| Arc::new(Semaphore::new(v))),
        }
    }
}

fn build_https_connector(tls_config: ClientConfig, http2: bool) -> HttpsConnector<HttpConnector> {
    let builder = HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1();

    if http2 {
        builder.enable_http2().build()
    } else {
        builder.build()
    }
}

/// Build a proxy connector using `proxies`, wrapping underlying `connector`,
/// and using `tls_config` to secure communications with the proxy.
fn build_proxy_connector<C>(
    proxies: &[Proxy],
    connector: C,
    tls_config: ClientConfig,
) -> ProxyConnector<C>
where
    C: TowerService<Uri> + Send,
    C::Response: hyper::rt::Read + hyper::rt::Write + Send + Unpin,
    C::Future: Send + 'static,
    C::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // The `unsecured()` constructor only skips loading the default TLS config, which
    // `set_tls` replaces.
    let mut proxy_connector = ProxyConnector::unsecured(connector);
    proxy_connector.extend_proxies(proxies.iter().cloned());
    proxy_connector.set_tls(Some(TlsConnector::from(Arc::new(tls_config))));
    proxy_connector
}

#[cfg(test)]
mod tests {
    use hyper_http_proxy::Intercept;

    use super::*;

    #[tokio::test]
    async fn test_default_builder() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let builder = HttpClientBuilder::https_with_system_roots().await?;

        assert_eq!(None, builder.max_redirects);
        assert!(builder.proxies.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_http2_option() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let mut builder = HttpClientBuilder::https_with_system_roots().await?;
        assert!(builder.http2);
        builder.with_http2(false);

        assert!(!builder.http2);
        Ok(())
    }

    #[tokio::test]
    async fn test_with_max_redirects_overrides_default() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let mut builder = HttpClientBuilder::https_with_system_roots().await?;
        builder.with_max_redirects(5);

        assert_eq!(5, builder.max_redirects.unwrap());
        Ok(())
    }

    #[tokio::test]
    async fn test_builder_with_proxy_adds_proxy() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let proxy = Proxy::new(Intercept::All, "http://localhost:12345".try_into()?);
        let mut builder = HttpClientBuilder::https_with_system_roots().await?;
        builder.with_proxy(proxy);

        assert_eq!(1, builder.proxies.len());
        Ok(())
    }

    #[tokio::test]
    async fn test_set_connect_timeout() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let mut builder = HttpClientBuilder::https_with_system_roots().await?;
        builder.with_connect_timeout(Some(Duration::from_millis(1000)));

        assert_eq!(
            Some(TimeoutConfig {
                connect_timeout: Some(Duration::from_millis(1000)),
                read_timeout: None,
                write_timeout: None,
            }),
            builder.timeout_config,
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_set_connect_and_read_timeouts() -> yak_error::Result<()> {
        yak_certs::certs::maybe_setup_cryptography();
        let mut builder = HttpClientBuilder::https_with_system_roots().await?;
        builder
            .with_connect_timeout(Some(Duration::from_millis(1000)))
            .with_read_timeout(Some(Duration::from_millis(2000)));
        assert_eq!(
            Some(TimeoutConfig {
                connect_timeout: Some(Duration::from_millis(1000)),
                read_timeout: Some(Duration::from_millis(2000)),
                write_timeout: None,
            }),
            builder.timeout_config,
        );
        Ok(())
    }
}
