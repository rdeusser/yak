# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


@yak_test()
async def test_target_call_stacks_default(yak: Yak) -> None:
    result = await yak.uquery(
        "--stack",
        "root//:test",
    )
    golden(
        output=result.stdout,
        rel_path="golden/uquery.stdout",
    )
    result = await yak.cquery(
        "--stack",
        "root//:test",
    )
    golden(
        output=result.stdout,
        rel_path="golden/cquery.stdout",
    )
