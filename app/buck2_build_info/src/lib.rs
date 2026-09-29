/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use buck2_util::late_binding::LateBinding;

pub struct Buck2BuildInfo {
    pub revision: Option<&'static str>,
}

pub static BUCK2_BUILD_INFO: LateBinding<Buck2BuildInfo> = LateBinding::new("BUCK2_BUILD_INFO");

/// Get the source control revision for this binary, if available. We provide this externally when
/// building yak for release.
pub fn revision() -> Option<&'static str> {
    BUCK2_BUILD_INFO
        .get()
        .ok()
        .and_then(|i| i.revision)
        .filter(|s| !s.is_empty())
}
