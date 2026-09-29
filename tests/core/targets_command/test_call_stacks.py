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


@yak_test()
async def test_target_call_stacks_json(yak: Yak) -> None:
    out = await yak.targets(
        "--stack",
        "--output-attribute=.*",
        "root//:test",
    )

    out = json.loads(out.stdout)
    call_stack = out[0]["yak.target_call_stack"]
    assert "stub" in call_stack
