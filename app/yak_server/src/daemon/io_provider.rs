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

use yak_common::cas_digest::CasDigestConfig;
use yak_common::io::IoProvider;
use yak_common::io::fs::FsIoProvider;
use yak_common::io::trace::TracingIoProvider;
use yak_core::fs::project::ProjectRoot;

pub async fn create_io_provider(
    project_fs: ProjectRoot,
    cas_digest_config: CasDigestConfig,
    trace_io: bool,
) -> yak_error::Result<Arc<dyn IoProvider>> {
    let io = FsIoProvider::new(project_fs, cas_digest_config);
    if trace_io {
        Ok(Arc::new(TracingIoProvider::new(Box::new(io))))
    } else {
        Ok(Arc::new(io))
    }
}
