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


OUTPUT_LIMIT_EXCEEDED = "(output limit exceeded)"


@yak_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_truncates_action_stderr(yak: Yak) -> None:
    """With a small output limit, action stderr should be truncated."""
    yak.set_env("YAK_CONSOLE_OUTPUT_LIMIT", "10")
    res = await yak.build("--console=simplenotty", "-v5", "//:noisy1")
    # The exceeded message should appear exactly once.
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr
    # With a 10-byte limit, most of the output should be suppressed.
    assert res.stderr.count("stderr line") <= 1, res.stderr


@yak_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_not_set_prints_all(yak: Yak) -> None:
    """Without the env var, all output should be printed."""
    res = await yak.build("--console=simplenotty", "-v5", "//:noisy1")
    assert OUTPUT_LIMIT_EXCEEDED not in res.stderr, res.stderr
    assert res.stderr.count("stderr line") == 10, res.stderr


@yak_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_global_across_actions(yak: Yak) -> None:
    """The limit is global: building two noisy targets should still only
    print the exceeded message once."""
    yak.set_env("YAK_CONSOLE_OUTPUT_LIMIT", "10")
    res = await yak.build("--console=simplenotty", "-v5", "//:noisy1", "//:noisy2")
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr


@yak_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_truncates_test_output(yak: Yak) -> None:
    """With a small output limit, test result output should be truncated."""
    yak.set_env("YAK_CONSOLE_OUTPUT_LIMIT", "10")
    # The test rule exits with 1 so that test result details (stderr) are
    # always printed, letting us verify the output limit applies to them.
    res = await expect_failure(
        yak.test("--console=simplenotty", "//:noisy_test"),
    )
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr
    assert res.stderr.count("test output line") <= 1, res.stderr
