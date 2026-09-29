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
async def test_replay(yak: Yak) -> None:
    await yak.build("//:EEE")
    replay = await yak.log("replay", "-v2")
    assert "//:EEE" in replay.stderr


@yak_test()
async def test_partial_result_replay(yak: Yak) -> None:
    # `audit cell` is an easy way to produce partial results
    res = await yak.audit("cell")
    res2 = await yak.log("replay")

    assert res.stdout != res2.stdout
    assert res2.stdout == ""
