/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_fs::paths::abs_norm_path::AbsNormPathBuf;
use yak_fs::paths::file_name::FileName;

/// `~/.yak/yakd/repo-path` directory.
#[derive(Debug, Clone, derive_more::Display)]
#[display("{}", path.display())]
pub struct DaemonDir {
    pub path: AbsNormPathBuf,
}

impl DaemonDir {
    /// Path to `yakd.info` file.
    pub fn yakd_info(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("yakd.info").unwrap())
    }

    /// Path to `yakd.stdout` file.
    pub fn yakd_stdout(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("yakd.stdout").unwrap())
    }

    /// Path to `yakd.stderr` file.
    pub fn yakd_stderr(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("yakd.stderr").unwrap())
    }

    /// Path to `yakd.pid` file.
    pub fn yakd_pid(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("yakd.pid").unwrap())
    }

    pub fn yakd_error_log(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("yakd.error.log").unwrap())
    }
}
