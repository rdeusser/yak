/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::str::FromStr;
use std::sync::Arc;

use gazebo::eq_chain;
use yak_error::YakErrorContext;
use yak_hash::StdYakHashMap;
use yak_util::env_vars::substitute_env_vars;

use crate::legacy_configs::configs::ConfigValue;
use crate::legacy_configs::configs::LegacyYakConfig;
use crate::legacy_configs::configs::LegacyYakConfigSection;
use crate::legacy_configs::configs::LegacyYakConfigValue;
use crate::legacy_configs::key::YakconfigKeyRef;
use crate::legacy_configs::view::LegacyYakConfigView;

/// Read the `[yak_metadata]` section from a `LegacyYakConfig` and resolve any `$VAR`
/// references. Entries whose env vars are not set are skipped with a warning.
pub fn parse_yakconfig_metadata(config: &LegacyYakConfig) -> StdYakHashMap<String, String> {
    let mut map = StdYakHashMap::default();
    let Some(section) = config.get_section("yak_metadata") else {
        return map;
    };
    for (key, value) in section.iter() {
        match substitute_env_vars(value.as_str()) {
            Ok(resolved) => {
                map.insert(key.to_owned(), resolved);
            }
            Err(e) => {
                tracing::warn!("Skipping [yak_metadata] key `{}`: {:#}", key, e);
            }
        }
    }
    map
}

impl LegacyYakConfigView for &LegacyYakConfig {
    fn get(&mut self, key: YakconfigKeyRef) -> yak_error::Result<Option<Arc<str>>> {
        Ok(LegacyYakConfig::get(self, key).map(|v| v.to_owned().into()))
    }
}

impl LegacyYakConfigSection {
    /// configs are equal if the data they resolve in is equal, regardless of the origin of the config
    pub(crate) fn compare(&self, other: &Self) -> bool {
        eq_chain!(
            self.values.len() == other.values.len(),
            self.values.iter().all(|(name, value)| other
                .values
                .get(name)
                .is_some_and(|other_val| other_val.as_str() == value.as_str()))
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, LegacyYakConfigValue<'_>)> {
        self.values
            .iter()
            .map(move |(key, value)| (key.as_str(), LegacyYakConfigValue { value }))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    pub fn get(&self, key: &str) -> Option<LegacyYakConfigValue<'_>> {
        self.values
            .get(key)
            .map(move |value| LegacyYakConfigValue { value })
    }
}

impl LegacyYakConfig {
    fn get_config_value(&self, key: YakconfigKeyRef) -> Option<&ConfigValue> {
        let YakconfigKeyRef { section, property } = key;
        self.0
            .values
            .get(section)
            .and_then(|s| s.values.get(property))
    }

    pub fn get(&self, key: YakconfigKeyRef) -> Option<&str> {
        self.get_config_value(key).map(|s| s.as_str())
    }

    /// Iterate all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, impl IntoIterator<Item = (&str, &str)>)> {
        self.0.values.iter().map(|(section, section_values)| {
            (
                section.as_str(),
                section_values
                    .values
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            )
        })
    }

    fn parse_impl<T: FromStr>(key: YakconfigKeyRef, value: &str) -> yak_error::Result<T>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        let YakconfigKeyRef { section, property } = key;
        value
            .parse()
            .map_err(yak_error::Error::from)
            .with_yak_error_context(|| {
                format!(
                    "Invalid value for yakconfig `{}.{}`: conversion to {} failed, value as `{}`",
                    section.to_owned(),
                    property.to_owned(),
                    std::any::type_name::<T>(),
                    value.to_owned(),
                )
            })
    }

    pub fn parse<T: FromStr>(&self, key: YakconfigKeyRef) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        self.get_config_value(key)
            .map(|s| {
                Self::parse_impl(key, s.as_str()).with_yak_error_context(|| {
                    format!("Defined {}", s.source.as_legacy_yak_config_location())
                })
            })
            .transpose()
    }

    pub fn parse_value<T: FromStr>(
        key: YakconfigKeyRef,
        value: Option<&str>,
    ) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        value.map(|s| Self::parse_impl(key, s)).transpose()
    }

    pub fn parse_list<T: FromStr>(&self, key: YakconfigKeyRef) -> yak_error::Result<Option<Vec<T>>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        Self::parse_list_value(key, self.get(key))
    }

    pub fn parse_list_value<T: FromStr>(
        key: YakconfigKeyRef,
        value: Option<&str>,
    ) -> yak_error::Result<Option<Vec<T>>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        /// A wrapper type so we can use .parse() on this.
        struct ParseList<T>(Vec<T>);

        impl<T> FromStr for ParseList<T>
        where
            T: FromStr,
        {
            type Err = <T as FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(
                    s.split(',').map(T::from_str).collect::<Result<_, _>>()?,
                ))
            }
        }

        Ok(Self::parse_value::<ParseList<T>>(key, value)?.map(|l| l.0))
    }

    pub fn sections(&self) -> impl Iterator<Item = &String> {
        self.0.values.keys()
    }

    pub fn all_sections(&self) -> impl Iterator<Item = (&String, &LegacyYakConfigSection)> + '_ {
        self.0.values.iter()
    }

    pub fn get_section(&self, section: &str) -> Option<&LegacyYakConfigSection> {
        self.0.values.get(section)
    }

    /// configs are equal if the data they resolve in is equal, regardless of the origin of the config
    pub(crate) fn compare(&self, other: &Self) -> bool {
        eq_chain!(
            self.0.values.len() == other.0.values.len(),
            self.0.values.iter().all(|(section_name, section)| {
                other
                    .0
                    .values
                    .get(section_name)
                    .is_some_and(|other_sec| other_sec.compare(section))
            })
        )
    }
}
