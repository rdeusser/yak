/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Dice operations for legacy configuration

use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::DiceProjectionComputations;
use dice::DiceTransactionUpdater;
use dice::EqualityBehavior;
use dice::InjectedKey;
use dice::Key;
use dice::NoValueSerialize;
use dice::OkPagableValueSerialize;
use dice::OpaqueValue;
use dice::PagableValueSerialize;
use dice::ProjectionKey;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use dupe::ResultDupedErrExt;
use pagable::Pagable;
use pagable::pagable_typetag;
use yak_core::cells::name::CellName;
use yak_error::YakErrorContext;
use yak_error::YakErrorOptionContext;
use yak_events::dispatch::get_dispatcher;

use crate::dice::cells::HasCellResolver;
use crate::legacy_configs::cells::ExternalYakconfigData;
use crate::legacy_configs::cells::YakConfigBasedCells;
use crate::legacy_configs::configs::LegacyYakConfig;
use crate::legacy_configs::key::YakconfigKeyRef;
use crate::legacy_configs::view::LegacyYakConfigView;

/// yakconfig view which queries yakconfig entry from DICE.
#[derive(Clone, Dupe)]
pub struct OpaqueLegacyYakConfigOnDice<'d> {
    config: Arc<OpaqueValue<'d, LegacyYakConfigForCellKey>>,
}

impl<'d> std::fmt::Debug for OpaqueLegacyYakConfigOnDice<'d> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyYakConfigOnDice")
            .field("config", &self.config)
            .finish()
    }
}

impl<'d> OpaqueLegacyYakConfigOnDice<'d> {
    pub fn lookup(
        &self,
        ctx: &mut DiceComputations<'d>,
        key: YakconfigKeyRef,
    ) -> yak_error::Result<Option<Arc<str>>> {
        let YakconfigKeyRef { section, property } = key;
        Ok(ctx.projection(
            &*self.config,
            &LegacyYakConfigPropertyProjectionKey {
                section: section.to_owned(),
                property: property.to_owned(),
            },
        )?)
    }

    pub fn view<'a>(&'a self, ctx: &'a mut DiceComputations<'d>) -> LegacyYakConfigOnDice<'a, 'd> {
        LegacyYakConfigOnDice { ctx, config: self }
    }
}

pub struct LegacyYakConfigOnDice<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    config: &'a OpaqueLegacyYakConfigOnDice<'d>,
}

impl LegacyYakConfigOnDice<'_, '_> {
    pub fn parse<T: FromStr>(&mut self, key: YakconfigKeyRef) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyYakConfig::parse_value(key, self.get(key)?.as_deref())
    }
}

impl std::fmt::Debug for LegacyYakConfigOnDice<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegacyYakConfigOnDice")
            .field("config", &self.config)
            .finish()
    }
}

impl LegacyYakConfigView for LegacyYakConfigOnDice<'_, '_> {
    fn get(&mut self, key: YakconfigKeyRef) -> yak_error::Result<Option<Arc<str>>> {
        self.config.lookup(self.ctx, key)
    }
}

pub trait HasInjectedLegacyConfigs<'d> {
    fn get_injected_external_yakconfig_data(
        &mut self,
    ) -> impl Future<Output = yak_error::Result<&'d ExternalYakconfigData>>;

    fn is_injected_external_yakconfig_data_key_set(
        &mut self,
    ) -> impl Future<Output = yak_error::Result<bool>>;
}

#[async_trait]
pub trait HasLegacyConfigs<'d> {
    /// Get yakconfigs.
    ///
    /// This operation does not record yakconfig as a dependency of current computation.
    /// Accessing specific yakconfig property, records that key as dependency.
    async fn get_legacy_config_on_dice(
        &mut self,
        cell_name: CellName,
    ) -> yak_error::Result<OpaqueLegacyYakConfigOnDice<'d>>;

    async fn get_legacy_root_config_on_dice(
        &mut self,
    ) -> yak_error::Result<OpaqueLegacyYakConfigOnDice<'d>>;

    /// Use this function carefully: a computation which fetches this key will be recomputed
    /// if any yakconfig property changes.
    ///
    /// Consider using `get_legacy_config_property` instead.
    async fn get_legacy_config_for_cell(
        &mut self,
        cell_name: CellName,
    ) -> yak_error::Result<&'d LegacyYakConfig>;

    async fn get_legacy_config_property(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<Arc<str>>>;

    async fn parse_legacy_config_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static;

    async fn parse_legacy_config_list_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<Vec<T>>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static;
}

pub trait SetLegacyConfigs {
    fn set_legacy_config_external_data(
        &mut self,
        overrides: ExternalYakconfigData,
    ) -> yak_error::Result<()>;

    fn set_none_legacy_config_external_data(&mut self) -> yak_error::Result<()>;
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
#[display("{:?}", self)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct LegacyExternalYakConfigDataKey;

impl InjectedKey for LegacyExternalYakConfigDataKey {
    type Value = Option<ExternalYakconfigData>;

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| x == y)
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

#[derive(Clone, Display, Debug, Hash, Eq, PartialEq, Allocative, Pagable)]
#[display("LegacyYakConfigForCellKey({})", self.cell_name)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct LegacyYakConfigForCellKey {
    cell_name: CellName,
}

#[async_trait]
impl Key for LegacyYakConfigForCellKey {
    type Value = yak_error::Result<LegacyYakConfig>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> yak_error::Result<LegacyYakConfig> {
        let cells = ctx.get_cell_resolver().await?;
        let this_cell = cells.get(self.cell_name)?;
        let config = YakConfigBasedCells::parse_single_cell_with_dice(ctx, this_cell.path())
            .await
            .with_yak_error_context(|| {
                format!("Computing legacy yakconfigs for cell `{}`", self.cell_name)
            })?;
        let config = config.filter_values(is_config_invisible_to_dice);

        let event = yak_data::CellHasNewConfigs {
            cell: self.cell_name.as_str().to_owned(),
        };
        get_dispatcher().instant_event(event);

        Ok(config)
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| match (x, y) {
            (Ok(x), Ok(y)) => x.compare(y),
            _ => false,
        })
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

/// The computation `LegacyYakConfigForCellKey` computation might encounter an error.
///
/// We can't return that error immediately, because we only compute the opaque value. We could
/// return the error when doing the projection to the yakconfig values, but that would result in us
/// increasing the size of the value returned from that computation. Instead, we'll use a different
/// projection key to extract just the error from the cell computation, and compute that when
/// constructing the `OpaqueLegacyYakConfigOnDice`.
#[derive(Debug, Display, Hash, Eq, PartialEq, Clone, Allocative, Pagable)]
#[pagable_typetag(dice::DiceProjectionDyn)]
struct LegacyYakConfigErrorKey();

impl ProjectionKey for LegacyYakConfigErrorKey {
    type DeriveFromKey = LegacyYakConfigForCellKey;
    type Value = Option<yak_error::Error>;

    fn compute(
        &self,
        config: &yak_error::Result<LegacyYakConfig>,
        _ctx: &DiceProjectionComputations,
    ) -> Option<yak_error::Error> {
        config.as_ref().err().cloned()
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| x.is_none() && y.is_none())
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        NoValueSerialize::<Self::Value>::new()
    }
}

#[derive(Debug, Display, Hash, Eq, PartialEq, Clone, Allocative, Pagable)]
#[display("{}.{}", section, property)]
#[pagable_typetag(dice::DiceProjectionDyn)]
struct LegacyYakConfigPropertyProjectionKey {
    section: String,
    property: String,
}

impl ProjectionKey for LegacyYakConfigPropertyProjectionKey {
    type DeriveFromKey = LegacyYakConfigForCellKey;
    type Value = Option<Arc<str>>;

    fn compute(
        &self,
        config: &yak_error::Result<LegacyYakConfig>,
        _ctx: &DiceProjectionComputations,
    ) -> Option<Arc<str>> {
        // See the comment in `LegacyYakConfigErrorKey` for why this is safe
        let config = config.as_ref().unwrap();
        config
            .get(YakconfigKeyRef {
                section: &self.section,
                property: &self.property,
            })
            .map(|s| s.to_owned().into())
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| x == y)
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        NoValueSerialize::<Self::Value>::new()
    }
}

impl<'d> HasInjectedLegacyConfigs<'d> for DiceComputations<'d> {
    async fn get_injected_external_yakconfig_data(
        &mut self,
    ) -> yak_error::Result<&'d ExternalYakconfigData> {
        self.compute(&LegacyExternalYakConfigDataKey).await?.as_ref().internal_error("Tried to retrieve LegacyExternalYakConfigDataKey from the graph, but key has None value")
    }

    async fn is_injected_external_yakconfig_data_key_set(&mut self) -> yak_error::Result<bool> {
        Ok(self
            .compute(&LegacyExternalYakConfigDataKey)
            .await?
            .is_some())
    }
}

pub fn inject_legacy_config_for_test(
    dice: &mut DiceTransactionUpdater,
    cell_name: CellName,
    configs: LegacyYakConfig,
) -> yak_error::Result<()> {
    inject_legacy_configs_for_test(dice, [(cell_name, configs)])
}

pub fn inject_legacy_configs_for_test(
    dice: &mut DiceTransactionUpdater,
    configs: impl IntoIterator<Item = (CellName, LegacyYakConfig)> + Send + Sync + 'static,
) -> yak_error::Result<()> {
    let configs = configs
        .into_iter()
        .map(|(cell_name, configs)| (LegacyYakConfigForCellKey { cell_name }, Ok(configs)))
        .collect::<Vec<_>>();
    dice.changed_to(configs)?;
    dice.changed_to([(LegacyExternalYakConfigDataKey, None)])?;
    Ok(())
}

#[async_trait]
impl<'d> HasLegacyConfigs<'d> for DiceComputations<'d> {
    async fn get_legacy_config_on_dice(
        &mut self,
        cell_name: CellName,
    ) -> yak_error::Result<OpaqueLegacyYakConfigOnDice<'d>> {
        let config = self
            .compute_opaque(&LegacyYakConfigForCellKey { cell_name })
            .await?;
        if let Some(error) = self.projection(&config, &LegacyYakConfigErrorKey())? {
            return Err(error);
        }
        Ok(OpaqueLegacyYakConfigOnDice {
            config: Arc::new(config),
        })
    }

    async fn get_legacy_root_config_on_dice(
        &mut self,
    ) -> yak_error::Result<OpaqueLegacyYakConfigOnDice<'d>> {
        let cell_resolver = self.get_cell_resolver().await?;
        self.get_legacy_config_on_dice(cell_resolver.root_cell())
            .await
    }

    async fn get_legacy_config_for_cell(
        &mut self,
        cell_name: CellName,
    ) -> yak_error::Result<&'d LegacyYakConfig> {
        self.compute(&LegacyYakConfigForCellKey { cell_name })
            .await?
            .as_ref()
            .duped_err()
    }

    async fn get_legacy_config_property(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<Arc<str>>> {
        self.get_legacy_config_on_dice(cell_name)
            .await?
            .lookup(self, key)
    }

    async fn parse_legacy_config_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<T>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static,
    {
        LegacyYakConfig::parse_value(
            key,
            self.get_legacy_config_property(cell_name, key)
                .await?
                .as_deref(),
        )
    }

    async fn parse_legacy_config_list_property<T: FromStr>(
        &mut self,
        cell_name: CellName,
        key: YakconfigKeyRef<'_>,
    ) -> yak_error::Result<Option<Vec<T>>>
    where
        yak_error::Error: From<<T as FromStr>::Err>,
        T: Send + Sync + 'static,
    {
        LegacyYakConfig::parse_list_value(
            key,
            self.get_legacy_config_property(cell_name, key)
                .await?
                .as_deref(),
        )
    }
}

impl SetLegacyConfigs for DiceTransactionUpdater {
    fn set_legacy_config_external_data(
        &mut self,
        data: ExternalYakconfigData,
    ) -> yak_error::Result<()> {
        let data = data.filter_values(is_config_invisible_to_dice);
        Ok(self.changed_to(vec![(LegacyExternalYakConfigDataKey, Some(data))])?)
    }

    fn set_none_legacy_config_external_data(&mut self) -> yak_error::Result<()> {
        Ok(self.changed_to(vec![(LegacyExternalYakConfigDataKey, None)])?)
    }
}

fn is_config_invisible_to_dice(key: &YakconfigKeyRef) -> bool {
    !CONFIGS_INVISIBLE_TO_DICE.contains(key)
}

/// A set of yakconfigs that are visibile outside of dice, but not within it. Importantly, changes
/// to these configs do not cause state invalidations.
// FIXME(JakobDegen): Error if someone tries to read any of these from in dice
const CONFIGS_INVISIBLE_TO_DICE: &[YakconfigKeyRef<'static>] = &[YakconfigKeyRef {
    section: "yak_re_client",
    property: "override_use_case",
}];

#[cfg(test)]
mod tests {
    use yak_cli_proto::ConfigOverride;

    use crate::legacy_configs::configs::testing::parse_with_config_args;

    #[test]
    fn config_equals() -> yak_error::Result<()> {
        let path = "test";
        let config1 = parse_with_config_args(
            &[("test", "[sec1]\na=b\n[sec2]\nx=y")],
            path,
            &[ConfigOverride::flag_no_cell("sec1.a=c")],
        )?;

        let config2 = parse_with_config_args(&[("test", "[sec1]\na=c\n[sec2]\nx=y")], path, &[])?;

        let config3 = parse_with_config_args(
            &[("test", "[sec1]\na=b\n[sec2]\nx=y")],
            path,
            &[ConfigOverride::flag_no_cell("sec1.d=e")],
        )?;

        assert!(config1.compare(&config1));
        assert!(config2.compare(&config2));
        assert!(config3.compare(&config3));
        assert!(config1.compare(&config2));
        assert!(!config1.compare(&config3));
        assert!(!config2.compare(&config3));

        Ok(())
    }
}
