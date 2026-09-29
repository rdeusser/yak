/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::ffi::OsString;
use std::path::Path;

use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use yak_error::YakErrorContext;
use yak_error::yak_error;

pub fn maybe_setup_cryptography() {
    setup_cryptography().ok();
}

pub fn setup_cryptography_or_fail() {
    setup_cryptography().unwrap();
}

fn setup_cryptography() -> std::result::Result<(), std::sync::Arc<rustls::crypto::CryptoProvider>> {
    // Note that all but the first call will fail, so we callers should only use
    // this function as early as possible in their lifetime
    // Note that the use of 'ring' here is arbitrary and should not be
    // taken as an intentional choice of cryptographic provider
    rustls::crypto::ring::default_provider().install_default()
}

/// Load system root certs, trying a few different methods to get a valid root
/// certificate store.
async fn load_system_root_certs() -> yak_error::Result<RootCertStore> {
    match find_root_ca_certs() {
        Some(path) => load_root_certs_from_path(Path::new(&path)).await,
        None => load_native_system_root_certs().await,
    }
}

async fn load_root_certs_from_path(path: &Path) -> yak_error::Result<RootCertStore> {
    let root_certs = load_certs(path)
        .await
        .with_yak_error_context(|| format!("Loading root certs from: {}", path.display()))?;
    root_cert_store_from_certs(root_certs)
}

async fn load_native_system_root_certs() -> yak_error::Result<RootCertStore> {
    let mut native_certs_results =
        tokio::task::spawn_blocking(rustls_native_certs::load_native_certs)
            .await
            .yak_error_context("Loading native system root certificates")?;

    let root_certs = if !native_certs_results.certs.is_empty() {
        Ok(native_certs_results.certs)
    } else {
        // Consider the last error to be indicative of the overall problem
        let native_certs_error = native_certs_results
            .errors
            .pop()
            .map(yak_error::Error::from)
            .unwrap_or(yak_error!(
                yak_error::ErrorTag::NoValidCerts,
                "No certs or cert errors"
            ));

        Err(native_certs_error.context("Error loading system root certificates native frameworks."))
    }?;
    root_cert_store_from_certs(root_certs)
}

fn root_cert_store_from_certs(
    root_certs: Vec<CertificateDer<'static>>,
) -> yak_error::Result<RootCertStore> {
    // According to [`rustls` documentation](https://docs.rs/rustls/latest/rustls/struct.RootCertStore.html#method.add_parsable_certificates),
    // it's better to only add parseable certs when loading system certs because
    // there are typically many system certs and not all of them can be valid. This
    // is pertinent for e.g. macOS which may have a lot of old certificates that may
    // not parse correctly.
    let mut roots = RootCertStore::empty();
    let (valid, invalid) = roots.add_parsable_certificates(root_certs);

    // But make sure we get at least _one_ valid cert, otherwise we legitimately won't be
    // able to make any connections via https.
    if valid == 0 {
        return Err(yak_error!(
            yak_error::ErrorTag::Environment,
            "Error loading system certs: unable to find any valid system certs"
        ));
    }
    tracing::debug!("Loaded {} valid system root certs", valid);
    tracing::debug!("Loaded {} invalid system root certs", invalid);
    Ok(roots)
}

pub async fn tls_config_with_system_roots() -> yak_error::Result<ClientConfig> {
    let system_roots = load_system_root_certs().await?;
    Ok(ClientConfig::builder()
        .with_root_certificates(system_roots)
        .with_no_client_auth())
}

// Load certs from the given path
async fn load_certs<P: AsRef<Path>>(
    cert_path: P,
) -> yak_error::Result<Vec<CertificateDer<'static>>> {
    let cert_path = cert_path.as_ref();

    let cert_data = tokio::fs::read(cert_path)
        .await
        .with_yak_error_context(|| {
            format!("Error reading certificate file `{}`", cert_path.display())
        })?;

    let cert_results: Vec<Result<CertificateDer, rustls_pki_types::pem::Error>> =
        CertificateDer::pem_reader_iter(&mut cert_data.as_slice()).collect();

    let certs: Result<Vec<CertificateDer<'static>>, rustls_pki_types::pem::Error> =
        cert_results.into_iter().collect();

    certs.with_yak_error_context(|| {
        format!("Error reading certificate file `{}`", cert_path.display())
    })
}

/// Find root CA certs.
///
/// Returns the path in `ROOT_CA_CERT_PATH` when it exists. Otherwise the caller
/// loads the platform's native root certificates.
fn find_root_ca_certs() -> Option<OsString> {
    match std::env::var_os("ROOT_CA_CERT_PATH") {
        Some(path) if Path::new(&path).exists() => Some(path),
        _ => None,
    }
}
