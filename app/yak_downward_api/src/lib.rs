/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The downward api for external processes. This crate defines a trait of downward api that yak
//! will need to handle as the process runner.

use tracing::Level;
use yak_hash::YakMutMap;

/// The API available to processes that yak will need to handle
#[async_trait::async_trait]
pub trait DownwardApi {
    /// indicates to print to the console at a specific log level
    async fn console(&self, level: Level, msg: String) -> yak_error::Result<()>;

    /// indicates to log at a specified level
    /// TODO consider if we should have structured log instead of a String message
    async fn log(&self, level: Level, msg: String) -> yak_error::Result<()>;

    /// reports an externally consumable event containing some data that will be untouched by yak
    async fn external(&self, data: YakMutMap<String, String>) -> yak_error::Result<()>;
}
