# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
@pytest.mark.parametrize(
    "rule, passes",
    [
        ("self//:pass1", True),
        ("self//:pass2", True),
        ("self//:pass3", True),
        ("self//:pass4", True),
        ("self//:fail1", False),
        ("self//:fail2", False),
        ("self//:fail3", False),
        ("self//:fail4", False),
        ("self//:fail5", False),
        ("self//:fail6", False),
    ],
)
async def test_audit_visibility(yak: Yak, rule: str, passes: bool) -> None:
    if passes:
        out = await yak.audit_visibility(rule)
        assert out.stdout == ""
    else:
        await expect_failure(
            yak.audit_visibility(rule),
            stderr_regex=f"not visible to `{rule}`",
        )
