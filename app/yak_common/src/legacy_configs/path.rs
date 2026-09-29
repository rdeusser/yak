/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_wrapper_common::DOT_YAKCONFIG_D;

pub(crate) enum ExternalConfigSource {
    // yakconfig file in the user's home directory
    UserFile(&'static str),

    // yakconfig folder in the user's home directory, assuming all files in this folder are yakconfig
    UserFolder(&'static str),

    // Global yakconfig file. Repo related config is not allowed
    GlobalFile(&'static str),

    // Global yakconfig folder, assuming all files in this folder are yakconfig. Repo related config is not allowed
    GlobalFolder(&'static str),
}

pub(crate) enum ProjectConfigSource {
    // yakconfig file in the cell relative to project root, such as .yakconfig or .yakconfig.local
    CellRelativeFile(&'static str),

    // yakconfig folder in the cell, assuming all files in this folder are yakconfig
    CellRelativeFolder(&'static str),
}

/// The default places from which yakconfigs are sourced.
///
/// Later entries take precedence over earlier ones, and project configs take precedence over
/// external configs.
pub(crate) static DEFAULT_EXTERNAL_CONFIG_SOURCES: &[ExternalConfigSource] = &[
    #[cfg(not(windows))]
    ExternalConfigSource::GlobalFolder("/etc/yakconfig.d"),
    #[cfg(not(windows))]
    ExternalConfigSource::GlobalFile("/etc/yakconfig"),
    // TODO: use %PROGRAMDATA% on Windows
    #[cfg(windows)]
    ExternalConfigSource::GlobalFolder("C:\\ProgramData\\yakconfig.d"),
    #[cfg(windows)]
    ExternalConfigSource::GlobalFile("C:\\ProgramData\\yakconfig"),
    ExternalConfigSource::UserFolder(DOT_YAKCONFIG_D),
    ExternalConfigSource::UserFile(DOT_YAKCONFIG_LOCAL),
];

pub(crate) static DEFAULT_PROJECT_CONFIG_SOURCES: &[ProjectConfigSource] = &[
    ProjectConfigSource::CellRelativeFolder(DOT_YAKCONFIG_D),
    ProjectConfigSource::CellRelativeFile(".yakconfig"),
    ProjectConfigSource::CellRelativeFile(DOT_YAKCONFIG_LOCAL),
];

pub(crate) static DOT_YAKCONFIG_LOCAL: &str = ".yakconfig.local";
