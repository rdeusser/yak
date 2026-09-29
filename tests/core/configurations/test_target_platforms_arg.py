# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_target_platforms_arg(yak: Yak) -> None:
    out = await yak.cquery(
        # Specifying platform without cell to make sure it is resolved against current cell
        "--target-platforms=//:p-clouds",
        "deps(//:the-test, 1)",
        rel_cwd=Path("subcell"),
    )
    stdout = re.sub(":p-clouds#[a-f0-9]+\\)", ":p-clouds#HASH)", out.stdout)
    assert (
        stdout
        == """\
subcell//:the-test (subcell//:p-clouds#HASH)
subcell//:t-clouds (subcell//:p-clouds#HASH)
"""
    )
