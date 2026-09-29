/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::str::FromStr;

use allocative::Allocative;
use yak_common::legacy_configs::configs::LegacyBuckConfig;
use yak_common::legacy_configs::key::BuckconfigKeyRef;

static BUCK2_RE_CLIENT_CFG_SECTION: &str = "yak_re_client";

/// Settings every remote execution configuration provides.
pub trait RemoteExecutionStaticMetadataImpl: Sized {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> yak_error::Result<Self>;
    /// Whether there is a CAS to talk to at all. Independent of any executor configuration: a
    /// command that runs nothing remotely still has one, and asks it about blobs.
    fn cas_configured(&self) -> bool;
    fn cas_semaphore_size(&self) -> usize;
    fn exec_semaphore_size(&self) -> usize;
    fn action_cache_semaphore_size(&self) -> usize;
}

/// Metadata that doesn't change between executions
#[derive(Clone, Debug, Default, Allocative)]
pub struct RemoteExecutionStaticMetadata(pub Buck2OssReConfiguration);

impl RemoteExecutionStaticMetadataImpl for RemoteExecutionStaticMetadata {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> yak_error::Result<Self> {
        Ok(Self(Buck2OssReConfiguration::from_legacy_config(
            legacy_config,
        )?))
    }

    fn cas_configured(&self) -> bool {
        self.0.cas_address.is_some()
    }

    fn cas_semaphore_size(&self) -> usize {
        // FIXME: make this configurable?
        1024
    }

    fn action_cache_semaphore_size(&self) -> usize {
        // FIXME: make this configurable?
        1024
    }

    fn exec_semaphore_size(&self) -> usize {
        self.0.execution_concurrency_limit.unwrap_or(400)
    }
}

/// The remote execution configuration, read from the `yak_re_client` yakconfig section.
#[derive(Clone, Debug, Default, Allocative)]
pub struct Buck2OssReConfiguration {
    /// Address for RBE Content Addresable Storage service (including bytestream uploads service).
    pub cas_address: Option<String>,
    /// Address for RBE Engine service (including capabilities service).
    pub engine_address: Option<String>,
    /// Address for RBE Action Cache service.
    pub action_cache_address: Option<String>,
    /// Whether to use TLS to interact with remote execution.
    pub tls: bool,
    /// Path to a CA certificates bundle. This must be PEM-encoded. If none is set, a default
    /// bundle will be used.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub tls_ca_certs: Option<String>,
    /// Path to a client certificate (and intermediate chain), as well as its associated private
    /// key. This must be PEM-encoded.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub tls_client_cert: Option<String>,
    /// HTTP headers to inject in all requests to RE. This is a comma-separated list of `Header:
    /// Value` pairs. Minimal validation of those headers is done here.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub http_headers: Vec<HttpHeader>,
    /// Whether to query capabilities from the RBE backend.
    pub capabilities: Option<bool>,
    /// The instance name to use in requests.
    pub instance_name: Option<String>,
    /// The max size for a GRPC message to be decoded.
    pub max_decoding_message_size: Option<usize>,
    /// The max cumulative blob size for `Read` and `BatchReadBlobs` methods.
    pub max_total_batch_size: Option<usize>,
    /// Maximum number of concurrent upload requests for each action.
    pub max_concurrent_uploads_per_action: Option<usize>,
    /// Maximum number of digests to ask about in a single
    /// `FindMissingBlobs` (a.k.a. `GetDigestsTtl`) RPC. Larger values
    /// reduce per-call wall-clock latency by issuing fewer round-trips,
    /// at the cost of bigger requests and more concurrent server load
    /// when many actions issue independent calls. Recommended to raise
    /// only in combination with `[yak] deduplicate_get_digests_ttl_calls`.
    pub find_missing_blobs_batch_size: Option<usize>,
    /// Time that digests are assumed to live in CAS after being touched.
    pub cas_ttl_secs: Option<i64>,
    /// Interval in seconds for HTTP/2 ping frames to detect stale connections.
    pub grpc_keepalive_time_secs: Option<u64>,
    /// Timeout in seconds for receiving HTTP/2 ping acknowledgement.
    pub grpc_keepalive_timeout_secs: Option<u64>,
    /// Whether to send HTTP/2 pings when connection is idle.
    pub grpc_keepalive_while_idle: Option<bool>,
    /// Maximum number of concurrent execution requests.
    pub execution_concurrency_limit: Option<usize>,
    /// Minimum number of HTTP/2 connections per host in the connection pool.
    pub min_connections: Option<usize>,
    /// Maximum number of HTTP/2 connections per host in the connection pool.
    pub max_connections: Option<usize>,
    /// Maximum concurrent streams per connection.
    pub max_concurrency_per_connection: Option<usize>,
}

#[derive(Clone, Debug, Default, Allocative)]
pub struct HttpHeader {
    pub key: String,
    pub value: String,
}

impl FromStr for HttpHeader {
    type Err = yak_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.splitn(2, ':');
        match (iter.next(), iter.next()) {
            (Some(key), Some(value)) => Ok(Self {
                key: key.trim().to_owned(),
                value: value.trim().to_owned(),
            }),
            _ => Err(yak_error::yak_error!(
                yak_error::ErrorTag::Input,
                "Invalid header (expect name and value separated by `:`): `{}`",
                s
            )),
        }
    }
}

impl Buck2OssReConfiguration {
    pub fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> yak_error::Result<Self> {
        // this is used for all three services by default, if given; if one of
        // them has an explicit address given as well though, use that instead
        let default_address: Option<String> = legacy_config.parse(BuckconfigKeyRef {
            section: BUCK2_RE_CLIENT_CFG_SECTION,
            property: "address",
        })?;

        Ok(Self {
            cas_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "cas_address",
                })?
                .or(default_address.clone()),
            engine_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "engine_address",
                })?
                .or(default_address.clone()),
            action_cache_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "action_cache_address",
                })?
                .or(default_address),
            tls: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "tls",
                })?
                .unwrap_or(true),
            tls_ca_certs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "tls_ca_certs",
            })?,
            tls_client_cert: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "tls_client_cert",
            })?,
            http_headers: legacy_config
                .parse_list(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "http_headers",
                })?
                .unwrap_or_default(), // Empty list is as good None.
            capabilities: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "capabilities",
            })?,
            instance_name: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "instance_name",
            })?,
            max_decoding_message_size: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_decoding_message_size",
            })?,
            max_total_batch_size: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_total_batch_size",
            })?,
            max_concurrent_uploads_per_action: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_concurrent_uploads_per_action",
            })?,
            find_missing_blobs_batch_size: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "find_missing_blobs_batch_size",
            })?,
            cas_ttl_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "cas_ttl_secs",
            })?,
            grpc_keepalive_time_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_time_secs",
            })?,
            grpc_keepalive_timeout_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_timeout_secs",
            })?,
            grpc_keepalive_while_idle: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_while_idle",
            })?,
            execution_concurrency_limit: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "execution_concurrency_limit",
            })?,
            min_connections: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "min_connections",
            })?,
            max_connections: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_connections",
            })?,
            max_concurrency_per_connection: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_concurrency_per_connection",
            })?,
        })
    }
}
