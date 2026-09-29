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


@yak_test()
async def test_action_error_handler_types(yak: Yak) -> None:
    await yak.bxl(
        "//:test_action_error_handler_types.bxl:test_action_error_handler_types"
    )


@yak_test()
async def test_output_when_no_error_handler_used(yak: Yak) -> None:
    failure = await expect_failure(
        yak.build("//:does_not_use_error_handler"),
    )

    assert "Action sub-errors produced by error handlers: <empty>" not in failure.stderr


@yak_test()
async def test_error_handler_succeed_on_nonetype(yak: Yak) -> None:
    await yak.build("//:error_handler_nonetype")


@yak_test()
async def test_output_for_error_handler_with_errorformat(yak: Yak) -> None:
    failure = await expect_failure(
        yak.build("//:error_handler_with_errorformat"),
    )

    assert "- [test_failure] main.rs:10 expected `;`, found `}`" in failure.stderr
    assert "manually created sub error" not in failure.stderr
