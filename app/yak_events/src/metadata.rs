/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Facts about the running binary and host that events and on-disk state record.
use std::env;
use std::sync::OnceLock;

use yak_hash::IntentionallyStdHashMap;

use crate::daemon_id::DaemonId;

/// `collect_with_extras` returns the result of `collect` merged with `extras`.
///
/// `extras` come from the `[yak_metadata]` yakconfig section. They only fill keys that `collect`
/// leaves empty, so configuration cannot replace fields such as `hostname` or `daemon_uuid`.
pub fn collect_with_extras(
    daemon: &DaemonId,
    extras: &IntentionallyStdHashMap<String, String>,
) -> IntentionallyStdHashMap<String, String> {
    let mut map = collect(daemon);
    for (k, v) in extras.iter() {
        map.entry(k.clone()).or_insert_with(|| v.clone());
    }
    map
}

/// `collect` returns the daemon id, the host name, the operating system and its version, the CPU
/// architecture, and the yak revision when the binary was built with one.
pub fn collect(daemon: &DaemonId) -> IntentionallyStdHashMap<String, String> {
    let mut map = IntentionallyStdHashMap::new();
    map.insert("daemon_uuid".to_owned(), daemon.to_string());
    if let Some(hostname) = hostname() {
        map.insert("hostname".to_owned(), hostname);
    }
    map.insert("os".to_owned(), os_type().to_owned());
    if let Some(version) = os_version() {
        map.insert("os_version".to_owned(), version);
    }
    map.insert("arch".to_owned(), env::consts::ARCH.to_owned());
    if let Some(rev) = yak_build_info::revision() {
        map.insert("yak_revision".to_owned(), rev.to_owned());
    }
    map
}

/// The operating system - "linux" "darwin" "windows" etc.
fn os_type() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

#[cfg(target_os = "windows")]
fn os_version() -> Option<String> {
    winver::WindowsVersion::detect().map(|v| v.to_string())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn os_version() -> Option<String> {
    sys_info::os_release().ok()
}

pub fn hostname() -> Option<String> {
    static CELL: OnceLock<Option<String>> = OnceLock::new();

    CELL.get_or_init(|| {
        hostname::get()
            .ok()
            .map(|res| res.to_string_lossy().into_owned())
    })
    .clone()
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn os_version_produces_reasonable_windows_version() {
        let data = collect(&DaemonId::new());
        // This logic used to use the `GetVersionExW` win32 API, which
        // always returns the value below on recent versions of windows. See
        // https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getversionexw
        // for more details.
        assert_ne!(data["os_version"], "6.2.9200");
        // This is true for both Windows 10 and Windows 11: https://learn.microsoft.com/en-us/windows/win32/sysinfo/operating-system-version
        assert!(data["os_version"].starts_with("10.0."));
    }
}

#[cfg(test)]
mod collect_tests {
    use super::*;

    #[test]
    fn collect_records_only_local_fields() {
        let daemon = DaemonId::new();
        let data = collect(&daemon);
        assert_eq!(data["daemon_uuid"], daemon.to_string());
        assert_eq!(data["arch"], env::consts::ARCH);
        let local_fields = [
            "daemon_uuid",
            "hostname",
            "os",
            "os_version",
            "arch",
            "yak_revision",
        ];
        for key in data.keys() {
            assert!(
                local_fields.contains(&key.as_str()),
                "unexpected metadata key `{key}`"
            );
        }
    }

    #[test]
    fn extras_do_not_replace_collected_fields() {
        let daemon = DaemonId::new();
        let extras = IntentionallyStdHashMap::from([
            ("daemon_uuid".to_owned(), "from-config".to_owned()),
            ("team".to_owned(), "build".to_owned()),
        ]);
        let data = collect_with_extras(&daemon, &extras);
        assert_eq!(data["daemon_uuid"], daemon.to_string());
        assert_eq!(data["team"], "build");
    }
}
