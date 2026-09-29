# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_build_transition_without_target_universe(yak: Yak) -> None:
    result = await yak.build_without_report(
        "root//:yak",
        "--target-platforms=root//:p",
        "--show-output",
    )

    lines = result.stdout.splitlines()
    # Just a single target is built and output
    assert 1 == len(lines)
    assert "root//:yak yak-out" in lines[0]


@yak_test()
async def test_build_transition_with_target_universe(yak: Yak) -> None:
    result = await yak.build_without_report(
        "root//:yak",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:yak",
        "--show-output",
    )

    lines = result.stdout.splitlines()
    # Just a single target is built and output
    assert 1 == len(lines)
    assert "root//:yak yak-out" in lines[0]
