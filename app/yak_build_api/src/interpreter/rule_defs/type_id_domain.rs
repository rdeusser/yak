/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! yak's [`TypeIdDomain`]s (providers, transitive sets). Kept in yak rather
//! than starlark-rust, which is yak-agnostic and only knows its own
//! `record`/`enum` domains.

use dupe::Dupe;
use starlark::values::typing::TypeIdDomain;

/// [`TypeIdDomain`]s for yak-defined nominal types.
#[derive(Copy, Clone, Dupe, Debug, Eq, PartialEq)]
pub(crate) enum YakTypeIdDomain {
    /// A user-defined `provider(...)` type.
    UserProvider,
    /// A transitive set definition.
    TransitiveSet,
    /// A builtin (Rust-defined) provider type — both its instance and callable
    /// types; the role is disambiguated by the identity passed to `from_identity`.
    BuiltinProvider,
    /// The singleton `Provider` type that matches any provider instance.
    ProviderSingleton,
}

impl TypeIdDomain for YakTypeIdDomain {
    fn tag(&self) -> &'static str {
        match self {
            YakTypeIdDomain::UserProvider => "yak.user_provider",
            YakTypeIdDomain::TransitiveSet => "yak.transitive_set",
            YakTypeIdDomain::BuiltinProvider => "yak.builtin_provider",
            YakTypeIdDomain::ProviderSingleton => "yak.provider_singleton",
        }
    }
}
