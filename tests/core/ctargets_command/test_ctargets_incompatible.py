# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test()
async def test_ctargets_incompatible(yak: Yak) -> None:
    result = await yak.ctargets(
        # This one will be omitted from the output because it is not compatible.
        "root//:triangle",
        # This one will be output.
        "root//:square",
        "--target-platforms=root//:rectangular",
    )
    stdout = _replace_hash(result.stdout)
    [line] = stdout.splitlines()
    assert line == "root//:square (root//:rectangular#<HASH>)"

    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:triangle (root//:rectangular#" in result.stderr
