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
async def test_ctargets_transition(yak: Yak) -> None:
    # This target does self-transition, and `ctargets` outputs both
    # forward node and forward target node.

    result = await yak.ctargets(
        "root//:candy",
        "--target-platforms=root//:p",
    )
    [line1, line2] = result.stdout.splitlines()
    line1 = _replace_hash(line1)
    line2 = _replace_hash(line2)
    assert [line1, line2] == [
        "root//:candy (root//:p#<HASH>)",
        "root//:candy (<clay>#<HASH>)",
    ]
