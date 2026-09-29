/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::environment::GlobalsBuilder;
use yak_util::late_binding::LateBinding;

/// Globals defined in `yak_build_api`.
pub static REGISTER_YAK_BUILD_API_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_BUILD_API_GLOBALS");

/// `__internal__`s defined in `yak_build_api`.
pub static REGISTER_YAK_BUILD_API_INTERNALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_BUILD_API_INTERNALS");

/// Globals defined in `yak_transitions` crate.
pub static REGISTER_YAK_TRANSITION_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_TRANSITION_GLOBALS");

/// Globals defined in `yak_action_impl` crate.
pub static REGISTER_YAK_ACTION_IMPL_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_ACTION_IMPL_GLOBALS");

/// Globals defined in `yak_anon_targets` crate.
pub static REGISTER_YAK_ANON_TARGETS_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_ANON_TARGETS_GLOBALS");

/// Globals defined in `yak_bxl` crate,
/// which are used to create the context for `.bxl` evaluation.
pub static REGISTER_YAK_BXL_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_BXL_GLOBALS");

/// Globals defined in `yak_cfg_constructor` crate.
pub static REGISTER_YAK_CFG_CONSTRUCTOR_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_YAK_CFG_CONSTRUCTOR_GLOBALS");
