# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import contextlib
import json
import platform
import re
import subprocess
import time
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env
from e2e_util.helper.utils import daemon_is_alive


@yak_test()
@env("YAK_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_inactivity_timeout(yak: Yak) -> None:
    #######################################################
    # Recommend running this test in opt mode
    # Otherwise the command that is run here
    # could take longer than 1 second to finish
    # causing this test to be flaky
    #######################################################

    # this will start the daemon
    status = await yak.server("--status")
    pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await yak.get_daemon_dir()

    time.sleep(1)  # 1 sec timeout

    # check it's dead
    for _ in range(20):
        time.sleep(1)
        if not daemon_is_alive(pid):
            result = await yak.status()
            assert "no yakd running" == result.stderr.splitlines()[-1]

            stderr = (daemon_dir / "yakd.stderr").read_text()
            assert "inactivity timeout elapsed" in stderr
            return

    raise AssertionError(f"Server with pid {pid} did not die in 20 seconds")


@yak_test()
async def test_server_endpoint_output(yak: Yak) -> None:
    result = await yak.server()
    stdout = result.stdout.strip()
    assert stdout.startswith("yakd.endpoint=")
    assert stdout.removeprefix("yakd.endpoint=")


@yak_test()
async def test_server_status_output(yak: Yak) -> None:
    result = await yak.server("--status")
    status = json.loads(result.stdout)
    pid = status["process_info"]["pid"]
    assert isinstance(pid, int)
    assert pid > 0


@yak_test()
async def test_server_status_snapshot_output(yak: Yak) -> None:
    result = await yak.server("--status", "--snapshot")
    status = json.loads(result.stdout)
    snapshot = status["snapshot"]
    assert snapshot is not None
    assert "yak_max_rss" in snapshot


@yak_test()
async def test_server_snapshot_requires_status(yak: Yak) -> None:
    await expect_failure(yak.server("--snapshot"), stderr_regex="--status")


@yak_test()
@pytest.mark.parametrize(
    "corrupt",
    ["not-json", '{"valid-json", "but-not-valid-data"}'],
)
async def test_corrupted_yakd_info(yak: Yak, corrupt: str) -> None:
    await yak.targets("//:rule")

    daemon_dir = await yak.get_daemon_dir()
    with open(f"{daemon_dir}/yakd.info") as f:
        # Check file exists and valid.
        json.load(f)

    # Kill that daemon now to avoid having making a mess and leaving 2 daemons
    # around.
    await yak.kill()

    with open(f"{daemon_dir}/yakd.info", "w") as f:
        f.write(corrupt)

    await yak.targets("//:rule")


@yak_test()
async def test_recovers_when_daemon_pid_cannot_be_killed(yak: Yak) -> None:
    # A stale yakd.info can name a pid we cannot kill (e.g. one reused by a
    # process owned by another user). yak must report the failed kill and start
    # a fresh daemon rather than aborting, which used to leave yakd.info in
    # place so every later invocation failed the same way.
    await yak.targets("//:rule")

    daemon_dir = await yak.get_daemon_dir()
    with open(f"{daemon_dir}/yakd.info") as f:
        info = json.load(f)

    # Kill the daemon so its endpoint stops accepting connections, forcing the
    # next invocation down the "could not connect, killing daemon" path.
    await yak.kill()

    # Point yakd.info at a pid that hard_kill_until cannot kill. An out-of-range
    # pid fails the kill deterministically on every platform, standing in for the
    # reused/foreign pid from the original bug report.
    info["pid"] = 9999999999999
    with open(f"{daemon_dir}/yakd.info", "w") as f:
        json.dump(info, f)

    # Recovers by starting a new daemon; this used to fail to connect entirely.
    result = await yak.targets("//:rule")
    assert "Failed to kill yakd" in result.stderr


@yak_test()
async def test_starts_a_daemon_after_kill_without_killing(yak: Yak) -> None:
    # `yak kill` leaves yakd.info naming a process that has exited, so the next
    # invocation has no daemon to kill.
    await yak.targets("//:rule")
    await yak.kill()
    result = await yak.targets("//:rule")
    assert "Starting new yak daemon" in result.stderr, result.stderr
    assert "killing daemon" not in result.stderr, result.stderr


@yak_test()
async def test_process_title(yak: Yak) -> None:
    await yak.build()  # Start the daemon
    status = await yak.status()
    status = json.loads(status.stdout)
    pid = status["process_info"]["pid"]

    if platform.system() == "Darwin":
        out = subprocess.check_output(["ps", "-o", "comm=", str(pid)]).strip()
        assert out.startswith(b"yakd[")
    elif platform.system() == "Linux":
        out = subprocess.check_output(["ps", "-o", "cmd=", str(pid)]).strip()
        assert out.startswith(b"yakd[")
    elif platform.system() == "Windows":
        # We guarantee no value there.
        pass
    else:
        raise Exception("Unknown platform")


@yak_test()
async def test_status_fields(yak: Yak) -> None:
    await yak.build()  # Start the daemon
    status = await yak.status()
    status = json.loads(status.stdout)
    assert status["valid_working_directory"]


@yak_test()
async def test_status_active_commands(yak: Yak) -> None:
    await yak.build()  # Start the daemon

    status = json.loads((await yak.status()).stdout)
    # `status` is a oneshot request, not a registered command, so an idle
    # daemon reports no active commands.
    assert status["active_commands"] == []

    async def run_build() -> None:
        await yak.build(":long_running", "--local-only", "--no-remote-cache")

    build_task = asyncio.create_task(run_build())
    try:
        # Generous budget: under heavy CI load, daemon warm-up plus the local
        # action launch can take a while to register as an active command.
        for _ in range(600):
            status = json.loads((await yak.status()).stdout)
            if status["active_commands"]:
                break
            if build_task.done():
                # Surface the real failure rather than timing out below.
                await build_task
                raise AssertionError(
                    "Build finished without appearing in active_commands"
                )
            await asyncio.sleep(0.1)
        else:
            raise AssertionError("Build never showed up in active_commands")

        [command] = status["active_commands"]
        assert command["trace_id"], "Active command should report its trace id"
        assert any("long_running" in arg for arg in command["argv"]), (
            f"Build argv should mention the target: {command['argv']}"
        )
        assert command["stats"] is not None
    finally:
        build_task.cancel()
        # Consume the task's outcome so an underlying error is not silently
        # dropped at teardown.
        await asyncio.gather(build_task, return_exceptions=True)


@yak_test()
async def test_status_all(yak: Yak) -> None:
    # this will start the daemons
    await yak.server()

    status = await yak.status()
    status = json.loads(status.stdout)
    pid = status["process_info"]["pid"]

    status_all = await yak.status("--all")
    status_all = json.loads(status_all.stdout)
    for status in status_all:
        if status["process_info"]["pid"] == pid:
            return
    raise Exception(
        f"yakd status for pid {pid} not found in {json.dumps(status_all, indent=2)}"
    )


@yak_test()
@env("YAK_LOG", "yak_client_ctx::daemon::client::kill=debug")
async def test_no_yakd_kills_existing_daemon(yak: Yak) -> None:
    await yak.audit("cell")  # Start the daemon
    result = await yak.audit("cell", "--no-yakd")  # Kill the existing daemon
    assert "Killing daemon with PID" in result.stderr


@yak_test()
async def test_yak_out_is_cache_dir(yak: Yak) -> None:
    await yak.targets(":")  # Start a daemon
    root = await yak.root()
    assert (
        (Path(root.stdout.strip()) / "yak-out" / "v2" / "CACHEDIR.TAG")
        .read_text(encoding="utf-8")
        .startswith("Signature: 8a477f597d28d172789f06886806bc55")
    )


@yak_test()
async def test_yak_out_is_go_module(yak: Yak) -> None:
    await yak.targets(":")  # Start a daemon
    root = await yak.root()
    go_mod = Path(root.stdout.strip()) / "yak-out" / "go.mod"
    assert go_mod.read_text(encoding="utf-8") == "module yak-out\n"


@yak_test()
async def test_prev_daemon_dir(yak: Yak) -> None:
    await yak.targets(":")  # Start a daemon
    await yak.kill()
    await yak.targets(":")  # Start another daemon

    def extract_pid(stderr: str) -> int:
        pid = [re.match(r".* PID: (\d+)", line) for line in stderr.splitlines()]
        pid = list(filter(None, pid))
        assert len(pid) == 1, pid[0]
        return int(pid[0].group(1))

    new_daemon_stderr = await yak.daemon_stderr()
    killed_daemon_stderr = await yak.prev_daemon_stderr()

    # check logs contain yakd pid and don't match
    assert extract_pid(new_daemon_stderr) != extract_pid(killed_daemon_stderr)

    assert "triggered shutdown: `yak kill` was invoked" in killed_daemon_stderr


@yak_test()
@env("YAK_TESTING_INACTIVITY_TIMEOUT", "true")
@env("YAKD_STARTUP_INIT_TIMEOUT", "20")
async def test_recovers_promptly_after_inactivity_shutdown(yak: Yak) -> None:
    # A daemon that retires on its inactivity timeout leaves yakd.info behind
    # naming a pid that is gone. The next invocation must notice that quickly and
    # start a fresh daemon; it used to be suspected of spending the whole startup
    # budget here, which would turn an idle daemon into a 90s CLIENT_STARTUP_TIMEOUT.
    status = await yak.server("--status")
    pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await yak.get_daemon_dir()

    for _ in range(20):
        time.sleep(1)
        if not daemon_is_alive(pid):
            break
    else:
        raise AssertionError(f"Server with pid {pid} did not die in 20 seconds")

    assert "inactivity timeout elapsed" in (daemon_dir / "yakd.stderr").read_text()
    assert (daemon_dir / "yakd.info").exists(), "stale yakd.info is the point"

    start = time.time()
    result = await yak.targets("//:rule")
    elapsed = time.time() - start

    assert "Starting new yak daemon" in result.stderr, result.stderr
    # Well inside the 20s budget. A regression into the timeout path fails the
    # command outright, so this only guards against getting slow but succeeding.
    assert elapsed < 15.0, f"took {elapsed:.2f}s to recover"


@yak_test()
@env("YAK_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_inactivity_shutdown_exits_with_a_command_in_flight(yak: Yak) -> None:
    # The inactivity timer is only reset when a command *starts*, so a long lived
    # streaming command lets the timeout fire underneath itself. The daemon then
    # cannot finish draining while the client holds the stream open, and it used
    # to sit alive forever - still accepting connections it would never answer,
    # so every later invocation on this isolation dir burned its startup budget.
    status = await yak.server("--status")
    pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await yak.get_daemon_dir()

    subscriber = await yak.subscribe()
    try:
        for _ in range(20):
            time.sleep(1)
            if (
                "inactivity timeout elapsed"
                in (daemon_dir / "yakd.stderr").read_text()
            ):
                break
        else:
            raise AssertionError("daemon never reached its inactivity timeout")

        for _ in range(15):
            if not daemon_is_alive(pid):
                break
            time.sleep(1)
        else:
            raise AssertionError(
                f"daemon {pid} announced shutdown but is still alive with a command in flight"
            )

        assert "Shutdown deadline exceeded" in (daemon_dir / "yakd.stderr").read_text()
    finally:
        # Exiting tears the stream down under the subscriber, so it reports a
        # broken connection. That is the deliberate trade: the command is
        # terminated rather than the daemon being left alive and unusable.
        with contextlib.suppress(YakException):
            await subscriber.__aexit__(None, None, None)
