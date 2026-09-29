/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::eval::SoftErrorHandler;
use yak_core::soft_error;
pub struct Buck2StarlarkSoftErrorHandler;

/// When starlark deprecates something, we propagate it to our `soft_error!` handler.
impl SoftErrorHandler for Buck2StarlarkSoftErrorHandler {
    fn soft_error(&self, category: &str, error: starlark::Error) -> Result<(), starlark::Error> {
        let error = yak_error::Error::from(error);
        soft_error!(&format!("starlark_rust_{category}"), error, quiet: true, hard_error: true)?;
        Ok(())
    }
}
