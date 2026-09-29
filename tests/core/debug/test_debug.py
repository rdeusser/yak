# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os.path
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_debug_crash(yak: Yak) -> None:
    # If the first operation immediately does a panic then we fail to connect.
    # While that's not great, having some panics is better than none, so test once after we spawn.
    await yak.build()
    result = await expect_failure(yak.debug("crash", "panic"))
    assert "explicitly requested panic" in result.stderr
    # Our crash output should include a stack trace.
    assert "stack backtrace:" in result.stderr


@yak_test()
async def test_debug_exe(yak: Yak) -> None:
    result = await yak.debug("exe")
    path = result.stdout.strip()
    assert os.path.exists(path)


@yak_test()
async def test_debug_allocative(yak: Yak, tmp_path: Path) -> None:
    # Start the server.
    await yak.uquery("root//:")

    file_path = tmp_path / "profile"

    output = await yak.debug("allocative", "--output", str(file_path))
    assert os.path.exists(f"{file_path}/flame.src")
    assert os.path.exists(f"{file_path}/flame.svg")
    assert "Allocative profile written to" in output.stderr

    await yak.debug("allocative")
    assert os.path.exists(yak.cwd / "allocative-out" / "flame.src")
    assert os.path.exists(yak.cwd / "allocative-out" / "flame.svg")


@yak_test()
async def test_debug_filestatus(yak: Yak) -> None:
    # Start the server.
    await yak.uquery("root//:")
    # FIXME(JakobDegen): `.` is an error
    output = await yak.debug("file-status", "YAK.fixture")
    assert "No mismatches detected" in output.stderr


@yak_test()
async def test_debug_flush_pgo_profile(yak: Yak) -> None:
    await yak.build()
    result = await yak.debug("flush-pgo-profile")
    assert "was not flushed" in result.stderr


@yak_test(skip_for_os=["windows", "darwin"])
async def test_thread_dump(yak: Yak) -> None:
    # Make sure we don't start a daemon if there isn't one
    await expect_failure(
        yak.debug("thread-dump"),
        stderr_regex="No running yak daemon",
    )
    # Start the daemon
    await yak.uquery("root//:")
    output = await yak.debug("thread-dump")
    assert "frame #0" in output.stdout
