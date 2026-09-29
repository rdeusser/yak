# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test(skip_for_os=["windows"])
async def test_run_action_timeout_expires(yak: Yak) -> None:
    await expect_failure(
        yak.build("//:slow_with_timeout"),
        stderr_regex="timed out after",
    )


@yak_test(skip_for_os=["windows"])
async def test_run_action_timeout_succeeds(yak: Yak) -> None:
    result = await yak.build("//:fast_with_timeout")
    output = result.get_build_report().output_for_target("//:fast_with_timeout")
    assert output.read_text().strip() == "hello"
