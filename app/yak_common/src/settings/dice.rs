/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dice::DiceData;
use dice::DiceDataBuilder;
use dupe::Dupe;

use crate::settings::YakSettings;

/// Reads the daemon's Yak settings from DICE global data. Unlike yakconfig
/// reads this is a plain lookup, not a tracked computation.
pub trait HasYakSettings {
    /// The settings the daemon started with; defaults when none were installed
    /// (in-process tests).
    fn get_yak_settings(&self) -> YakSettings;
}

pub trait SetYakSettings {
    fn set_yak_settings(&mut self, settings: YakSettings);
}

impl HasYakSettings for DiceData {
    fn get_yak_settings(&self) -> YakSettings {
        match self.get::<YakSettings>() {
            Ok(settings) => settings.dupe(),
            Err(_) => YakSettings::default(),
        }
    }
}

impl SetYakSettings for DiceDataBuilder {
    fn set_yak_settings(&mut self, settings: YakSettings) {
        self.set(settings)
    }
}
