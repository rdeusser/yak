# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import os
import signal
from collections.abc import Callable
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException, YakResult, ExitCode
from e2e_util.api.process import Process
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import read_invocation_record


async def _test_cancellation_helper(
    yak: Yak,
    tmp_path: Path,
    runner: Callable[[Yak, list[str]], Process[YakResult, YakException]],
) -> None:
    """
    This test starts a test that writes its PID to a file then runs for 60
    seconds. We test cancellation by sending a CTRL+C as soon as a test
    starts. We then check that the process exited, and that nothing else
    started (or if anything did, that they stopped).
    """
    pid_path = tmp_path / "pids"
    pid_path.mkdir()
    record_path = tmp_path / "record.json"
    opts = [
        "-c",
        f"test.pids={pid_path}",
        "-c",
        "test.duration=60",
        "--unstable-write-invocation-record",
        str(record_path),
    ]
    await yak.audit("providers", ":slow", *opts)
    command = runner(yak, [*opts, "--local-only"])

    command = await command.start()

    for _i in range(30):
        await asyncio.sleep(1)
        pids = os.listdir(pid_path)
        if pids:
            break
    else:
        raise Exception("Commands never started")

    command.send_signal(signal.SIGINT)
    await command.communicate()  # Wait for the command to exit

    # Give stuff time to settle, PIDS don't necessarily disappear
    # instantly. Also, verify that we are not starting more tests.
    await asyncio.sleep(5)

    # At this point, nothing should be alive.
    pids = os.listdir(pid_path)
    for pid in pids:
        try:
            os.kill(int(pid), 0)
        except OSError:
            pass
        else:
            raise Exception(f"PID existed: {pid}")

    record = read_invocation_record(record_path)
    assert record["exit_code"] == ExitCode.SIGNAL_INTERRUPT.value
    assert record["exit_result_name"] == "SIGNAL_INTERRUPT"


@yak_test()
async def test_cancellation(yak: Yak, tmp_path: Path) -> None:
    await _test_cancellation_helper(
        yak, tmp_path, lambda yak, opts: yak.build(*opts, ":slow")
    )


@yak_test()
async def test_cancellation_bxl(yak: Yak, tmp_path: Path) -> None:
    await _test_cancellation_helper(
        yak, tmp_path, lambda yak, opts: yak.bxl(*opts, "//build.bxl:build")
    )
