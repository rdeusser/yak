# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
from pathlib import Path
from typing import Optional

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException, BuildResult
from e2e_util.api.process import Process
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


async def wait_for_action_start(
    task: asyncio.Task[tuple[Optional[int], str]], event_log: Path
) -> None:
    async def wait() -> None:
        while True:
            if task.done():
                try:
                    exit_code, stderr = task.result()
                except Exception as error:
                    raise AssertionError(
                        "Build failed before starting an action"
                    ) from error
                raise AssertionError(
                    f"Build exited before starting an action ({exit_code=}): {stderr}"
                )
            try:
                if '"ActionExecution"' in event_log.read_text():
                    return
            except FileNotFoundError:
                pass
            await asyncio.sleep(0.1)

    try:
        await asyncio.wait_for(wait(), timeout=30)
    except TimeoutError as error:
        raise AssertionError(
            f"Timed out waiting for an ActionExecution event in {event_log}"
        ) from error


@yak_test()
async def test_exit_when_different_state(yak: Yak) -> None:
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--exit-when=differentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = yak.build(
        "-c",
        "foo.bar=2",
        "--exit-when=differentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_preemptible_always(yak: Yak, same_state: bool) -> None:
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = yak.build(
        "-c",
        # We expect to ALWAYS preempt commands, to prevent blocking new callees
        "foo.bar=1" if same_state else "foo.bar=2",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@yak_test(write_invocation_record=True)
async def test_preemptible_logged(yak: Yak) -> None:
    res = await yak.targets(
        "--preemptible=always",
        ":",
    )
    record = res.invocation_record()
    assert record["preemptible"] == "ALWAYS"


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_preemptible_on_different_state(
    yak: Yak, same_state: bool
) -> None:
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--preemptible=ondifferentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = yak.build(
        "-c",
        # We expect to ALWAYS preempt commands, to prevent blocking new callees
        "foo.bar=1" if same_state else "foo.bar=2",
        "--preemptible=ondifferentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    if same_state:
        # No preempt when state is the same
        assert len(done) == 0
        assert len(pending) == 2
    else:
        assert len(done) == 1
        assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_does_not_start_when_daemon_busy(
    yak: Yak, same_state: bool, tmp_path: Path
) -> None:
    """Test that an incoming command with --exit-when=notidle does not start if there's another command running."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    event_log = tmp_path / "first.json-lines"
    a = yak.build(
        "-c",
        "foo.bar=1",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
        "--event-log",
        str(event_log),
    )
    task_a = asyncio.create_task(process(a))
    await wait_for_action_start(task_a, event_log)

    b = yak.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_does_not_gets_preempted(
    yak: Yak, same_state: bool, tmp_path: Path
) -> None:
    """Test that a running command with --exit-when=notidle continues running even with an incoming command."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    event_log = tmp_path / "first.json-lines"
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
        "--event-log",
        str(event_log),
    )
    task_a = asyncio.create_task(process(a))
    await wait_for_action_start(task_a, event_log)

    # Start another command without the flag (default is --preemptible=never)
    b = yak.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 0
    assert len(pending) == 2


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_preemptible_exit_when_not_idle_gets_preempted(
    yak: Yak, same_state: bool, tmp_path: Path
) -> None:
    """Test that a running command with --exit-when=notidle and --preemptible gets preempted with an incoming command."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    event_log = tmp_path / "first.json-lines"
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
        "--event-log",
        str(event_log),
    )
    task_a = asyncio.create_task(process(a))
    await wait_for_action_start(task_a, event_log)

    # Start another command without the flag (default is --preemptible=never)
    b = yak.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_multiple_exit_when_not_idle_commands(
    yak: Yak, same_state: bool, tmp_path: Path
) -> None:
    """
    Test that a running command with --exit-when=notidle does NOT get preempted by an incoming command that
    also has --exit-when=notidle set.
    """

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    event_log = tmp_path / "first.json-lines"
    a = yak.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
        "--event-log",
        str(event_log),
    )
    task_a = asyncio.create_task(process(a))
    await wait_for_action_start(task_a, event_log)

    b = yak.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@yak_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_after_command_exits(
    yak: Yak, same_state: bool
) -> None:
    """ """

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, YakException],
    ) -> tuple[Optional[int], str]:
        result = await p
        return (result.process.returncode, result.stderr)

    a = yak.build(
        "-c",
        "foo.bar=1",
        ":short_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Wait for the first command to actually finish. A fixed delay here made this test observe an
    # active command under load even though it intends to test the state after command exit.
    task_a = asyncio.create_task(process(a))
    await asyncio.wait_for(task_a, timeout=10)

    for _attempt in range(100):
        try:
            result_b = await yak.build(
                "-c",
                "foo.bar=1" if same_state else "foo.bar=2",
                "--exit-when=notidle",
                ":short_running_target",
                "--local-only",
                "--no-remote-cache",
            )
            break
        except YakException as error:
            if error.process.returncode != 4 or "daemon is busy" not in error.stderr:
                raise
            await asyncio.sleep(0.1)
    else:
        raise AssertionError("Concurrency state did not become idle after command exit")

    exit_code_a, stderr_a = task_a.result()
    assert "daemon is busy" not in stderr_a
    assert exit_code_a == 0
    assert "daemon is busy" not in result_b.stderr
    assert result_b.process.returncode == 0
