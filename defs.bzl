# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//decls:common.bzl", "buck")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup")

def _buck2_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    """
    Puts the client binary (`buck2`) and the daemon binary (`buck2-daemon`)
    in one directory, where the client-only build looks for the daemon.
    """
    target_is_windows = ctx.attrs._target_os_type[OsLookup].os == Os("windows")

    binary_extension = ".exe" if target_is_windows else ""
    buck2_binary = "buck2" + binary_extension
    buck2_daemon_binary = "buck2-daemon" + binary_extension

    copied_dir = {}
    materialisations = []

    buck2 = ctx.attrs.buck2[DefaultInfo].default_outputs[0]
    copied_dir[buck2_daemon_binary] = buck2
    materialisations.extend(ctx.attrs.buck2[DefaultInfo].other_outputs)

    buck2_client = ctx.attrs.buck2_client[DefaultInfo].default_outputs[0]
    copied_dir[buck2_binary] = buck2_client
    materialisations.extend(ctx.attrs.buck2_client[DefaultInfo].other_outputs)

    out = ctx.actions.copied_dir("out", copied_dir, has_content_based_path = False)

    return [DefaultInfo(out, other_outputs = materialisations), RunInfo(cmd_args(out.project("buck2" + binary_extension), hidden = materialisations))]

buck2_bundle = rule(
    impl = _buck2_bundle_impl,
    attrs = {
        "buck2": attrs.dep(),
        "buck2_client": attrs.dep(),
        "_target_os_type": buck.target_os_type_arg(),
    },
)

def _pagable_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    val = refs.val[ConstraintValueInfo]
    new_cfg = ConfigurationInfo(
        constraints = platform.configuration.constraints | {val.setting.label: val},
        values = platform.configuration.values,
    )
    return PlatformInfo(
        label = platform.label,
        configuration = new_cfg,
    )

_pagable_transition = transition(
    impl = _pagable_transition_impl,
    refs = {
        "val": "//starlark-rust/starlark:pagable[enabled]",
    },
)

def _pagable_alias_impl(ctx: AnalysisContext) -> list[Provider]:
    return ctx.attrs.actual.providers

# Builds `actual` with the `pagable` constraint of the starlark crate enabled.
pagable_transition_alias = rule(
    impl = _pagable_alias_impl,
    attrs = {
        "actual": attrs.dep(),
    },
    cfg = _pagable_transition,
)
