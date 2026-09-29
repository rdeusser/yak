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


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_enables_cgroup(yak: Yak) -> None:
    """When min_version_for_gated_status is set to a version <= the binary's
    DAEMON_CGROUP_VERSION, resource control should be enabled (status =
    if_available)."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        # Version 1 is the current DAEMON_CGROUP_VERSION, so this should enable.
        yakconfig.write("min_version_for_gated_status = 1\n")

    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is not None


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gated_default_status_overrides_gated_default(yak: Yak) -> None:
    """When the version gate passes, version_gated_default_status overrides the
    default gated status of if_available."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        yakconfig.write("min_version_for_gated_status = 1\n")
        yakconfig.write("version_gated_default_status = off\n")

    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is None


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_explicit_status_overrides_gated_one(yak: Yak) -> None:
    """When the explicit status is present it takes precedence."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        yakconfig.write("status = off\n")
        yakconfig.write("min_version_for_gated_status = 1\n")
        yakconfig.write("version_gated_default_status = required\n")

    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is None


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_disables_cgroup_when_version_too_high(yak: Yak) -> None:
    """When min_version_for_gated_status is set to a version higher than the
    binary's DAEMON_CGROUP_VERSION, resource control should remain off."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        # Version 9999 is higher than any DAEMON_CGROUP_VERSION, so this should not enable.
        yakconfig.write("min_version_for_gated_status = 9999\n")

    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is None


@yak_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_not_set_status_off(yak: Yak) -> None:
    """When min_version_for_gated_status is not set and status is off,
    resource control should be off."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        yakconfig.write("status = off\n")

    snapshot = await start_daemon_and_get_snapshot(yak)
    assert snapshot["allprocs_cgroup"] is None


async def start_daemon_and_get_snapshot(yak: Yak) -> dict[str, typing.Any]:
    await yak.targets(":")
    status_result = await yak.status("--snapshot")
    status_data = json.loads(status_result.stdout)
    return status_data["snapshot"]
