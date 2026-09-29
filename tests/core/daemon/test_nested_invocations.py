# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import typing
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


def nested_yak_args(yak: Yak) -> typing.List[str]:
    return [
        "-c",
        f"nested.yak_path={yak.path_to_executable}",
    ]


# A nested invocation fails even when it shares the daemon state, because
# `nested_invocation_same_dice_state` is a hard error.
@yak_test()
async def test_same_state(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:nested_normal", *nested_yak_args(yak)),
        stderr_regex="Recursive invocation of yak, which is discouraged, but will probably work \\(using the same state\\)",
    )


@yak_test(allow_soft_errors=True)
async def test_different_state_error(yak: Yak, tmp_path: Path) -> None:
    # FIXME(JakobDegen): Nested invocations seem to have buggy behavior around writing the event
    # logs, so `log show` and friends don't work without this
    log = tmp_path / "logfile.json-lines"
    await expect_failure(
        yak.build(
            "-c",
            "some.config=Val",
            "root//:nested_normal",
            "--event-log",
            str(log),
            *nested_yak_args(yak),
        ),
        stderr_regex="Failed to build 'root//:nested_normal",
    )
    res = await yak.log("what-ran", "--failed", "--show-std-err", str(log))
    assert "Recursive invocation of yak, with a different state" in res.stdout


@yak_test(allow_soft_errors=True)
async def test_trace_io_mismatch(yak: Yak, tmp_path: Path) -> None:
    log = tmp_path / "logfile.json-lines"
    await expect_failure(
        yak.build(
            "root//:nested_trace",
            "--event-log",
            str(log),
            *nested_yak_args(yak),
        ),
        stderr_regex="Failed to build 'root//:nested_trace",
    )
    res = await yak.log("what-ran", "--failed", "--show-std-err", str(log))
    assert (
        "daemon constraint mismatch during nested invocation: Trace IO mismatch"
        in res.stdout
    )
