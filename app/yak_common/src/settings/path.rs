/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub(crate) static DOT_YAKSETTINGS: &str = ".yaksettings.toml";
pub(crate) static DOT_YAKSETTINGS_LOCAL: &str = ".yaksettings.local.toml";

pub(crate) enum SettingsSource {
    RepoRootFile(&'static str),
    HomeFile(&'static str),
}

/// Ordered lowest to highest priority.
pub(crate) static DEFAULT_SETTINGS_SOURCES: &[SettingsSource] = &[
    SettingsSource::RepoRootFile(DOT_YAKSETTINGS),
    SettingsSource::HomeFile(DOT_YAKSETTINGS_LOCAL),
    SettingsSource::RepoRootFile(DOT_YAKSETTINGS_LOCAL),
];
