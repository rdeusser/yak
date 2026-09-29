# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import json
import os
import signal

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test

# The SHA-256 digest and size of `src`.
TEST_DIGEST = "283c4dc0d0072198f15eafa9569200005cdc63871fe0b4a258ee83692afb51d0:14"


@yak_test()
async def test_restart_requires_no_stdout(yak: Yak) -> None:
    res = await yak.targets("//:stage0", env={"FORCE_WANT_RESTART": "true"})
    assert res.stdout.count("//:stage0") == 1


@yak_test()
async def test_restart(yak: Yak) -> None:
    # Normally shows once.
    res = await expect_failure(yak.targets("//:invalid"))
    assert res.stderr.count("Unknown target `invalid`") == 1

    # But if we force a restart...
    res = await expect_failure(
        yak.targets("//:invalid", env={"FORCE_WANT_RESTART": "true"})
    )
    assert res.stderr.count("Unknown target `invalid`") == 2


@yak_test(allow_soft_errors=True)
async def test_restart_materializer_corruption(yak: Yak) -> None:
    stage1 = "//:stage1"
    res = await yak.build(stage1)
    out = res.get_build_report().output_for_target(stage1)

    # Now we remove this file (which comes to us via RE)
    # Only way to get it back is by killing the materializer state.
    os.unlink(out)

    res = await yak.build("//:stage2")
    assert "Your command will now restart" in res.stderr


@pytest.mark.remote_execution
@yak_test(allow_soft_errors=True)
async def test_restart_cas_missing(yak: Yak) -> None:
    # Make sure yak is not running.
    await yak.kill()

    # Start a daemon with the `src` file tombstoned. This means we cannot download it from RE.
    await yak.build(env={"YAK_TEST_TOMBSTONED_DIGESTS": TEST_DIGEST})

    # Now build //:stage2. yak must try to download the file, fail, then
    # restart the daemon.
    res = await yak.build("//:stage2")
    assert "Your command will now restart" in res.stderr

    # TODO: We should also handle the case where the top-level artifact is what
    # fails to download (i.e. build stage1 here instead).


@yak_test(
    allow_soft_errors=True,
    skip_for_os=["windows"],
)
async def test_restart_forkserver_crash(yak: Yak) -> None:
    # Start the daemon
    await yak.build()

    # Kill its forkserver.
    forkserver_pid = json.loads((await yak.status()).stdout)["forkserver_pid"]
    assert forkserver_pid is not None
    os.kill(forkserver_pid, signal.SIGKILL)

    # Wait for its forkserver to exit.
    for _ in range(10):
        try:
            os.kill(forkserver_pid, 0)
        except OSError:
            break
        else:
            await asyncio.sleep(1)

    # Now build a thing and check we restart
    res = await yak.build("//:stage2")
    assert "Your command will now restart" in res.stderr


@pytest.mark.remote_execution
@yak_test()
async def test_restart_disabled(yak: Yak) -> None:
    # Ensure no daemon
    await yak.kill()

    with open(yak.cwd / ".yakconfig", "a") as f:
        f.write("[yak]\nrestarter = false")

    result = await expect_failure(
        yak.build(
            "//:stage2",
            env={"YAK_TEST_TOMBSTONED_DIGESTS": TEST_DIGEST},
        ),
    )
    assert "Your command will now restart" not in result.stderr


@yak_test(write_invocation_record=True)
async def test_trace_id(yak: Yak) -> None:
    trace_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"

    # But if we force a restart...
    res = await expect_failure(
        yak.targets(
            "//:invalid",
            env={"FORCE_WANT_RESTART": "true", "YAK_WRAPPER_UUID": trace_id},
        )
    )
    record = res.invocation_record()
    assert record["trace_id"] != trace_id
    assert record["restarted_trace_id"] == trace_id
    assert record["should_restart"] is False
