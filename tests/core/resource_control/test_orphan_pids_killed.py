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
from e2e_util.helper.utils import filter_events, random_string


@yak_test(skip_for_os=["windows", "darwin"], disable_daemon_cgroup=False)
async def test_orphan_pids_killed(yak: Yak) -> None:
    await yak.build(
        "root//:spawn_orphan",
        "--no-remote-cache",
        "--local-only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    events = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "OrphanProcessesKilled",
    )

    assert len(events) > 0, "Expected at least one OrphanProcessesKilled instant event"

    orphan_processes = events[0]["orphan_processes"]
    assert len(orphan_processes) > 0, (
        f"Expected at least one orphan process, got: {orphan_processes}"
    )

    # The orphan should be either 'setsid' or 'sleep' that escaped the process group.
    # setsid execs into sleep, so depending on timing we might see either one.
    comms = [p["comm"] for p in orphan_processes]
    assert any("setsid" in c or "sleep" in c for c in comms), (
        f"Expected to find a 'setsid' or 'sleep' orphan process, got comms: {comms}"
    )


@yak_test(skip_for_os=["windows", "darwin"], disable_daemon_cgroup=False)
async def test_no_orphan_same_pg_timeout(yak: Yak) -> None:
    # Build a target that spawns a background process in the same process
    # group. The action has a short timeout, so it will be cancelled via
    # killpg, which kills the background process too. Cgroup cleanup should
    # find no remaining processes, so no OrphanProcessesKilled event.
    await expect_failure(
        yak.build(
            "root//:spawn_same_pg_timeout",
            "--no-remote-cache",
            "--local-only",
            "-c",
            f"test.cache_buster={random_string()}",
        ),
        stderr_regex="timed out after",
    )

    events = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "OrphanProcessesKilled",
    )

    assert len(events) == 0, (
        f"Expected no OrphanProcessesKilled events for same-process-group child, got: {events}"
    )
