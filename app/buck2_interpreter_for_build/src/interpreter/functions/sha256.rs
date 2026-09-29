/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use sha2::Digest;
use sha2::Sha256;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;

/// Contains functions that we include in all contexts.
#[starlark_module]
pub(crate) fn register_sha256(builder: &mut GlobalsBuilder) {
    /// Computes a sha256 digest for a string. Returns the hex representation of the digest.
    ///
    /// ```python
    /// sha256("yak is the best build system") == "2d42784f6132dd8329ac739b188097943893e333055a94e2ca33eb1ba9421ae2"
    /// ```
    fn sha256(#[starlark(require = pos)] val: &str) -> starlark::Result<String> {
        let hash = Sha256::digest(val.as_bytes());
        Ok(hex::encode(hash))
    }
}

#[cfg(test)]
mod tests {

    use starlark::assert::Assert;

    use crate::interpreter::functions::sha256::register_sha256;

    #[test]
    fn test_sha256() {
        let mut a = Assert::new();
        a.globals_add(register_sha256);
        a.eq(
            "sha256('123')",
            "'a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3'",
        );
    }
}
