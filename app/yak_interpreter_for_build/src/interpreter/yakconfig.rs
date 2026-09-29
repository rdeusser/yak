/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use dice::DiceComputations;
use hashbrown::HashTable;
use starlark::any::ProvidesStaticType;
use starlark::collections::Hashed;
use starlark::eval::Evaluator;
use starlark::values::StringValue;
use starlark::values::Trace;
use yak_common::legacy_configs::configs::LegacyYakConfig;
use yak_common::legacy_configs::dice::OpaqueLegacyYakConfigOnDice;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_core::soft_error;

use crate::interpreter::extra_value::InterpreterExtraValue;

#[derive(Debug, Trace, Allocative)]
struct YakConfigEntry<'v> {
    section: Hashed<String>,
    key: Hashed<String>,
    value: Option<StringValue<'v>>,
}

/// The `read_config` results of one module by `(section, key)`: a repeated read costs one table
/// lookup and hands out the same string. Kept in the module's extra value because the strings
/// are values of the module.
#[derive(Default, Debug, ProvidesStaticType, Trace, Allocative)]
pub(crate) struct YakConfigsCache<'v> {
    /// Hash map by `(section, key)` pair, so we do one table lookup per request.
    /// So we hash the `key` even if the section does not exist,
    /// but this is practically not an issue, because keys usually come with cached hash.
    current_cell: RefCell<HashTable<YakConfigEntry<'v>>>,
    root_cell: RefCell<HashTable<YakConfigEntry<'v>>>,
}

pub trait YakConfigsViewForStarlark {
    fn read_current_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>>;

    fn read_root_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>>;
}

/// Version of cell yakconfig optimized for fast query from `read_config` Starlark function.
pub(crate) struct LegacyYakConfigsForStarlark<'a> {
    configs_view: RefCell<&'a mut (dyn YakConfigsViewForStarlark + 'a)>,
}

impl<'a> fmt::Debug for LegacyYakConfigsForStarlark<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyYakConfigForStarlark")
            .finish_non_exhaustive()
    }
}

impl<'a> LegacyYakConfigsForStarlark<'a> {
    // `section` or `key` 32 bit hashes are well swizzled,
    // but concatenation of them into 64 bit integer is not.
    // This function tries to fix that.
    fn mix_hashes(a: u32, b: u32) -> u64 {
        fn murmur3_mix64(mut x: u64) -> u64 {
            x ^= x >> 33;
            x = x.wrapping_mul(0xff51afd7ed558ccd);
            x ^= x >> 33;
            x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
            x ^= x >> 33;
            x
        }

        murmur3_mix64(((a as u64) << 32) | (b as u64))
    }

    /// Constructor.
    pub(crate) fn new(
        configs_view: &'a mut (dyn YakConfigsViewForStarlark + 'a),
    ) -> LegacyYakConfigsForStarlark<'a> {
        LegacyYakConfigsForStarlark {
            configs_view: RefCell::new(configs_view),
        }
    }

    fn get_impl<'v>(
        &self,
        section: Hashed<&str>,
        key: Hashed<&str>,
        from_root_cell: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> yak_error::Result<Option<StringValue<'v>>> {
        let hash = Self::mix_hashes(section.hash().get(), key.hash().get());

        let cache = &InterpreterExtraValue::get(eval.module())?.yakconfigs;
        let mut cache = if from_root_cell {
            cache.root_cell.borrow_mut()
        } else {
            cache.current_cell.borrow_mut()
        };
        if let Some(e) = cache.find(hash, |e| {
            e.section.key() == section.key() && e.key.as_str() == *key.key()
        }) {
            return Ok(e.value);
        }

        let key_ref = YakconfigKeyRef {
            section: section.key(),
            property: key.key(),
        };
        let value = {
            let mut configs_view = self.configs_view.borrow_mut();
            if from_root_cell {
                configs_view.read_root_cell_config(key_ref)?
            } else {
                configs_view.read_current_cell_config(key_ref)?
            }
        }
        // The frozen heap, so that freezing the module does not copy the strings; the module
        // keeps them either way.
        .map(|v| eval.frozen_heap(|fh, edge| edge.rebrand(fh.alloc_str(&v))));

        cache.insert_unique(
            hash,
            YakConfigEntry {
                section: Hashed::new_unchecked(section.hash(), (*section.key()).to_owned()),
                key: Hashed::new_unchecked(key.hash(), (*key.key()).to_owned()),
                value,
            },
            |e| Self::mix_hashes(e.section.hash().get(), e.key.hash().get()),
        );

        Ok(value)
    }

    /// Find the yakconfig entry.
    pub(crate) fn current_cell_get<'v>(
        &self,
        section: StringValue,
        key: StringValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> yak_error::Result<Option<StringValue<'v>>> {
        // Note here we reuse the hashes of `section` and `key`,
        // if `read_config` is called repeatedly with the same constant arguments:
        // `StringValue` caches the hashes.
        self.get_impl(section.get_hashed_str(), key.get_hashed_str(), false, eval)
    }

    pub(crate) fn root_cell_get<'v>(
        &self,
        section: StringValue,
        key: StringValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> yak_error::Result<Option<StringValue<'v>>> {
        // Note here we reuse the hashes of `section` and `key`,
        // if `read_config` is called repeatedly with the same constant arguments:
        // `StringValue` caches the hashes.
        self.get_impl(section.get_hashed_str(), key.get_hashed_str(), true, eval)
    }
}

pub(crate) struct ConfigsOnDiceViewForStarlark<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    yakconfig: OpaqueLegacyYakConfigOnDice<'d>,
    root_yakconfig: OpaqueLegacyYakConfigOnDice<'d>,
}

impl<'a, 'd> ConfigsOnDiceViewForStarlark<'a, 'd> {
    pub(crate) fn new(
        ctx: &'a mut DiceComputations<'d>,
        yakconfig: OpaqueLegacyYakConfigOnDice<'d>,
        root_yakconfig: OpaqueLegacyYakConfigOnDice<'d>,
    ) -> Self {
        Self {
            ctx,
            yakconfig,
            root_yakconfig,
        }
    }
}

impl YakConfigsViewForStarlark for ConfigsOnDiceViewForStarlark<'_, '_> {
    fn read_current_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>> {
        read_config_and_report_deprecated(self.ctx, &self.yakconfig, key)
    }

    fn read_root_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>> {
        read_config_and_report_deprecated(self.ctx, &self.root_yakconfig, key)
    }
}

#[derive(Debug, yak_error::Error)]
#[error("{} is no longer used. {}", .0, .1)]
#[yak(tag = Input)]
struct DeprecatedConfigError(String, Arc<str>);

fn read_config_and_report_deprecated<'d>(
    ctx: &mut DiceComputations<'d>,
    config: &OpaqueLegacyYakConfigOnDice<'d>,
    key: YakconfigKeyRef,
) -> yak_error::Result<Option<Arc<str>>> {
    let result = config.lookup(ctx, key)?;
    let property = format!("{}.{}", key.section, key.property);

    let key = YakconfigKeyRef {
        section: "deprecated_config",
        property: &property,
    };
    let msg = config.lookup(ctx, key)?;
    if let Some(msg) = msg {
        // soft error category can only contain ascii lowercese characters
        let section = sanitize_category_part(key.section);
        let prop = sanitize_category_part(key.property);

        soft_error!(
            format!("deprecated_config_{section}_{prop}").as_str(),
            DeprecatedConfigError(property, msg).into(),
            quiet: true,
            hard_error: true
        )?;
    }
    Ok(result)
}

fn sanitize_category_part(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_lowercase() || *c == '_')
        .collect::<String>()
}

pub struct LegacyConfigsViewForStarlark {
    current_cell_config: LegacyYakConfig,
    root_cell_config: LegacyYakConfig,
}

impl LegacyConfigsViewForStarlark {
    pub(crate) fn new(yakconfig: LegacyYakConfig, root_yakconfig: LegacyYakConfig) -> Self {
        Self {
            current_cell_config: yakconfig,
            root_cell_config: root_yakconfig,
        }
    }
}

impl YakConfigsViewForStarlark for LegacyConfigsViewForStarlark {
    fn read_current_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>> {
        Ok(self
            .current_cell_config
            .get(key)
            .map(|v| v.to_owned().into()))
    }

    fn read_root_cell_config(
        &mut self,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>> {
        Ok(self.root_cell_config.get(key).map(|v| v.to_owned().into()))
    }
}
