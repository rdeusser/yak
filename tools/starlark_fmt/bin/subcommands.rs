/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod diff;
mod fmt;
mod stdin;

use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(crate) use diff::diff_file;
pub(crate) use fmt::format_files;
use rayon::prelude::*;
use starlark_fmt_lib::Config;
use starlark_fmt_lib::FormattedSource;
use starlark_fmt_lib::format_source;
pub(crate) use stdin::format_stdin;
use tracing::info_span;

pub(crate) struct ProcessedFile<'a> {
    pub(crate) path: &'a Path,
    /// Source after normalizing line endings (`\r\n` → `\n`).
    pub(crate) source: String,
    pub(crate) formatted: String,
}

impl<'a> ProcessedFile<'a> {
    pub(crate) fn new(path: &'a Path, config: &Config) -> anyhow::Result<Self> {
        if path.is_dir() {
            return Err(anyhow::anyhow!(
                "{}: path is a directory, expected a file",
                path.display()
            ));
        }

        let raw = info_span!("read_file").in_scope(|| {
            fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))
        })?;
        let FormattedSource {
            normalized: source,
            formatted,
        } = format_source(&raw, config, path)?;

        Ok(Self {
            path,
            source,
            formatted,
        })
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.formatted != self.source
    }
}

pub(crate) fn process_files<F>(files: &[PathBuf], process_fn: F) -> Vec<(PathBuf, anyhow::Error)>
where
    F: Fn(&Path) -> anyhow::Result<()> + Sync,
{
    files
        .par_iter()
        .filter_map(|path| {
            info_span!("process_file").in_scope(|| {
                process_fn(path.as_path())
                    .err()
                    .map(|err| (path.clone(), err))
            })
        })
        .collect()
}
