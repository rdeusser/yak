/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;

use crate::legacy_configs::configs::LegacyBuckConfig;
use crate::legacy_configs::key::BuckconfigKeyRef;

/// yakconfig trait.
///
/// There are two implementations:
/// * simple implementation which is backed by a yakconfig object, used in tests
/// * DICE-backed implementation which records a dependency on yakconfig property in DICE
pub trait LegacyBuckConfigView: Debug {
    fn get(&mut self, key: BuckconfigKeyRef) -> yak_error::Result<Option<Arc<str>>>;

    fn parse<T: FromStr>(&mut self, key: BuckconfigKeyRef) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyBuckConfig::parse_value(key, self.get(key)?.as_deref())
    }

    fn parse_list<T: FromStr>(&mut self, key: BuckconfigKeyRef) -> yak_error::Result<Option<Vec<T>>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyBuckConfig::parse_list_value(key, self.get(key)?.as_deref())
    }
}
