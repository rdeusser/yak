# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import os
from pathlib import Path
from tempfile import TemporaryDirectory

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakResult
from e2e_util.yak_workspace import yak_test, env
from e2e_util.helper.utils import filter_events

pytestmark = pytest.mark.needs_binary("USE_SOME_MEMORY_BIN")


def _configure(yak: Yak, kill_and_retry: bool) -> None:
    with open(yak.cwd / ".yakconfig.local", "w") as f:
        f.write("[yak_resource_control]\n")
        if kill_and_retry:
            f.write("preferred_action_suspend_strategy = kill_and_retry\n")
        else:
            f.write("preferred_action_suspend_strategy = cgroup_freeze\n")


def _use_some_memory_args(yak: Yak, temp: TemporaryDirectory[str]) -> list[str]:
    return [
        "--show-full-simple-output",
        "-c",
        f"use_some_memory.path={os.environ['USE_SOME_MEMORY_BIN']}",
        "-c",
        f"start_marker_files.path={temp.name}",
        "--no-remote-cache",
        "--local-only",
    ]


async def _check_suspends(  # noqa C901
    yak: Yak,
    kill_and_retry: bool,
    temp: TemporaryDirectory[str],
    res: YakResult,
) -> int:
    # First check the reported suspensions
    actions = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
    )
    reported_suspends = {}
    num_suspended_actions = 0
    for action in actions:
        if action["name"]["category"] != "memory_allocating_actions":
            continue
        ident = action["name"]["identifier"]
        command_meta = action["commands"][-1]["details"]["metadata"]
        if command_meta["suspend_duration"] is not None:
            num_suspended_actions += 1
            if kill_and_retry:
                count = command_meta["suspend_count"]
                assert count is not None
                reported_suspends[ident] = count
            else:
                # Json representation of a duration is number of us
                duration = command_meta["suspend_duration"] / 1000
                assert duration is not None
                reported_suspends[ident] = duration
        else:
            reported_suspends[ident] = None

    if kill_and_retry:
        total_detected_kills = 0
        expected_kills = 0
        for ident, count in reported_suspends.items():
            detected_starts = len((Path(temp.name) / ident).read_text().splitlines())
            if count is None:
                assert detected_starts == 1
            else:
                # We can't quite assert that the kills were observed by the action because sometimes
                # we might kill the thing before it gets there. So add them up and assert that we're
                # close enough
                total_detected_kills += detected_starts - 1
                expected_kills += count
        assert total_detected_kills >= expected_kills - 2
    else:
        paths = Path(res.stdout.strip()).read_text().splitlines()
        paths = [yak.cwd / p for p in paths]
        assert len(paths) == len(reported_suspends)

        # Then compare them to the detected suspensions
        for p in paths:
            contents = p.read_text()
            ident = p.name
            total_duration: int | None = None
            for line in contents.splitlines():
                if line.startswith("freeze_detected_ms "):
                    this_duration = int(line[len("freeze_detected_ms ") :])
                    total_duration = (total_duration or 0) + this_duration
            if total_duration is not None:
                assert abs(reported_suspends[ident] - total_duration) < 500
            else:
                assert (reported_suspends[ident] or 0) < 500

    return num_suspended_actions


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@env("YAK_HARD_ERROR", "panic")
@pytest.mark.parametrize("kill_and_retry", [True, False])
async def test_action_suspend(
    yak: Yak,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(yak, kill_and_retry)
    res = await yak.build_without_report(
        ":sleep_10",
        *_use_some_memory_args(yak, temp),
    )

    await _check_suspends(yak, kill_and_retry, temp, res)


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@env("YAK_HARD_ERROR", "panic")
@pytest.mark.parametrize("kill_and_retry", [True, False])
async def test_action_suspend_stress_test(
    yak: Yak,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(yak, kill_and_retry)
    await yak.build(
        ":very_fast_100",
        *_use_some_memory_args(yak, temp),
    )


# Only kill_and_retry is exercised. Deterministic memory pressure requires swap to be disabled
# for actions (see `memory_swap_max_actions` in the data `.yakconfig`); otherwise the overshoot
# is paged out cheaply and pressure never crosses the suspension threshold. cgroup_freeze is
# incompatible with that: freezing an action does not release its memory, so with swap off the
# still-running action stays throttled above memory.high and never makes progress, deadlocking the
# build. There is no swap setting that both builds pressure (needs swap off) and lets a freeze
# relieve it (needs swap on), so this scenario cannot be tested deterministically for freeze.
@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@pytest.mark.parametrize("kill_and_retry", [True])
async def test_suspend_one_of_two(
    yak: Yak,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(yak, kill_and_retry)

    res = await yak.build_without_report(
        ":two_mutually_incompatible",
        *_use_some_memory_args(yak, temp),
    )

    num_suspends = await _check_suspends(yak, kill_and_retry, temp, res)
    assert num_suspends == 1
