/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

/// GoPlatform pairs a GOOS and GOARCH with the prelude constraints that select it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoPlatform {
    pub goos: &'static str,
    pub goarch: &'static str,
    pub os: &'static str,
    pub cpu: &'static str,
}

/// The platforms that the cell resolves each package's dependencies for, in the order of the
/// `go list` runs it takes.
pub const PLATFORMS: &[GoPlatform] = &[
    GoPlatform {
        goos: "darwin",
        goarch: "amd64",
        os: "prelude//os:macos",
        cpu: "prelude//cpu:x86_64",
    },
    GoPlatform {
        goos: "darwin",
        goarch: "arm64",
        os: "prelude//os:macos",
        cpu: "prelude//cpu:arm64",
    },
    GoPlatform {
        goos: "linux",
        goarch: "amd64",
        os: "prelude//os:linux",
        cpu: "prelude//cpu:x86_64",
    },
    GoPlatform {
        goos: "linux",
        goarch: "arm64",
        os: "prelude//os:linux",
        cpu: "prelude//cpu:arm64",
    },
    GoPlatform {
        goos: "windows",
        goarch: "amd64",
        os: "prelude//os:windows",
        cpu: "prelude//cpu:x86_64",
    },
    GoPlatform {
        goos: "windows",
        goarch: "arm64",
        os: "prelude//os:windows",
        cpu: "prelude//cpu:arm64",
    },
];
