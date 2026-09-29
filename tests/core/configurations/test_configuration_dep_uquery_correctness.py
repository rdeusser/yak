# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


async def check_has_uquery_path(
    yak: Yak, target: str, dep: str, expect_fail: bool = False
) -> None:
    result = await yak.uquery(
        f"somepath({target}, {dep})",
    )
    path = result.stdout.splitlines()
    # Apparently, configuration deps never show up in `somepath`. Interesting.
    assert len(path) == 0

    result = await yak.uquery(
        f"deps({target})",
        "-a",
        "yak.deps",
        "-a",
        "yak.configuration_deps",
    )
    all_deps = [
        d
        for node in json.loads(result.stdout).values()
        for deps in node.values()
        for d in deps
    ]
    if expect_fail:
        assert dep not in all_deps
    else:
        assert dep in all_deps


@yak_test()
async def test_default_target_platform(yak: Yak) -> None:
    await check_has_uquery_path(yak, ":with_custom_dtp", "root//:base")


@yak_test()
async def test_configured_dep_platform(yak: Yak) -> None:
    await check_has_uquery_path(yak, ":stub_configured", "root//:base")


@yak_test()
async def test_transition_dep_refs(yak: Yak) -> None:
    # FIXME(JakobDegen): Bug.
    await check_has_uquery_path(
        yak, ":pre_out_transition", "root//:cat", expect_fail=True
    )

    # FIXME(JakobDegen): Bug.
    await check_has_uquery_path(
        yak, ":post_out_transition", "root//:cat", expect_fail=True
    )

    await check_has_uquery_path(yak, ":pre_out_transition_vnew", "root//:transition")

    await check_has_uquery_path(yak, ":pre_inc_transition_vnew", "root//:transition")


@yak_test()
async def test_select_keys(yak: Yak) -> None:
    await check_has_uquery_path(yak, ":with_select", "root//:cat")
