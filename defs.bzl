# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//decls:common.bzl", "yak")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup")

def _yak_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    """
    Puts the client binary (`yak`) and the daemon binary (`yak-daemon`)
    in one directory, where the client-only build looks for the daemon.
    """
    target_is_windows = ctx.attrs._target_os_type[OsLookup].os == Os("windows")

    binary_extension = ".exe" if target_is_windows else ""
    yak_binary = "yak" + binary_extension
    yak_daemon_binary = "yak-daemon" + binary_extension

    copied_dir = {}
    materialisations = []

    yak = ctx.attrs.yak[DefaultInfo].default_outputs[0]
    copied_dir[yak_daemon_binary] = yak
    materialisations.extend(ctx.attrs.yak[DefaultInfo].other_outputs)

    yak_client = ctx.attrs.yak_client[DefaultInfo].default_outputs[0]
    copied_dir[yak_binary] = yak_client
    materialisations.extend(ctx.attrs.yak_client[DefaultInfo].other_outputs)

    out = ctx.actions.copied_dir("out", copied_dir, has_content_based_path = False)

    return [DefaultInfo(out, other_outputs = materialisations), RunInfo(cmd_args(out.project("yak" + binary_extension), hidden = materialisations))]

yak_bundle = rule(
    impl = _yak_bundle_impl,
    attrs = {
        "yak": attrs.dep(),
        "yak_client": attrs.dep(),
        "_target_os_type": yak.target_os_type_arg(),
    },
)
