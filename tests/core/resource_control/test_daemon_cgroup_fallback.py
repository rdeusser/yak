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
async def test_if_available_fallback_no_user_session(yak: Yak) -> None:
    """When status=if_available and systemd-run --user can't connect to the
    user session bus, the daemon should start successfully without cgroups
    instead of failing with DAEMON_STARTUP_FAILED."""

    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak_resource_control]\n")
        yakconfig.write("status = if_available\n")

    # Strip the env vars that systemd-run --user needs to find the D-Bus
    # session bus. This simulates the VS Code 3p extension sandbox which
    # filters these out via an allowlist.
    yak._env.pop("DBUS_SESSION_BUS_ADDRESS", None)
    yak._env.pop("XDG_RUNTIME_DIR", None)

    # The daemon should start successfully (fallback to no cgroup).
    await yak.targets(":")

    # Verify daemon is running without cgroups.
    status_result = await yak.status("--snapshot")
    status_data = json.loads(status_result.stdout)
    snapshot: dict[str, typing.Any] = status_data["snapshot"]
    assert snapshot["allprocs_cgroup"] is None
