# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import typing

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_metrics_cgroup_no_resource_control(yak: Yak) -> None:
    write_config(yak, resource_control=False)
    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is None
    assert snapshot["forkserver_actions_cgroup"] is None


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_metrics_cgroup_resource_control(yak: Yak) -> None:
    write_config(yak, resource_control=True)
    snapshot = await start_daemon_and_get_snapshot(yak)
    # Daemon should have allocated at least 500KB of anon memory
    assert snapshot["allprocs_cgroup"]["anon"] >= (
        snapshot["forkserver_actions_cgroup"]["anon"] + 500000
    )
    assert (
        snapshot["allprocs_cgroup"]["file"]
        >= snapshot["forkserver_actions_cgroup"]["file"]
    )
    assert (
        snapshot["allprocs_cgroup"]["kernel"]
        >= snapshot["forkserver_actions_cgroup"]["kernel"]
    )


@yak_test(
    skip_for_os=["darwin", "windows"],
)
async def test_cgroup_path_tag(yak: Yak) -> None:
    await yak.targets(":")
    events = await filter_events(yak, "Event", "data", "Instant", "data", "SystemInfo")
    assert len(events) >= 1
    path = events[0]["daemon_cgroup_slice_path"]
    assert path is not None
    assert path.startswith("/sys/fs/cgroup/")


def write_config(yak: Yak, *, resource_control: bool) -> None:
    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        yakconfig.write(f"status = {'required' if resource_control else 'off'}\n")


async def start_daemon_and_get_snapshot(yak: Yak) -> dict[str, typing.Any]:
    # Start the daemon
    await yak.targets(":")

    # Get the snapshot
    status_result = await yak.status("--snapshot")
    status_data = json.loads(status_result.stdout)
    snapshot = status_data["snapshot"]
    return snapshot
