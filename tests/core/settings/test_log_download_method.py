# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
from typing import Any

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


async def _log_download_method(yak: Yak) -> Any:
    result = await yak.status()
    config = json.loads(result.stdout)["daemon_constraints"]["daemon_startup_config"]
    return json.loads(config)["log_download_method"]


def _write_yakconfig_log_url(yak: Yak, value: str) -> None:
    with open(yak.cwd / ".yakconfig", "a") as f:
        f.write(f"\n[yak]\nlog_url = {value}\n")


def _write_home_local_settings(yak: Yak, settings: str) -> None:
    home = yak.get_settings_home_dir()
    (home / ".yaksettings.local.toml").write_text(settings)


@yak_test()
async def test_settings_override_yakconfig(yak: Yak) -> None:
    _write_yakconfig_log_url(yak, "abc.com")
    (yak.cwd / ".yaksettings.toml").write_text(
        '[log_download]\nlog_url = "test.com"\n'
    )

    await yak.server()
    assert await _log_download_method(yak) == {"Curl": "test.com"}


@yak_test()
async def test_log_url_fallback_to_yakconfig(yak: Yak) -> None:
    await yak.server()
    assert await _log_download_method(yak) == "None"

    _write_yakconfig_log_url(yak, "test.com")

    await yak.server()
    assert await _log_download_method(yak) == {"Curl": "test.com"}


@yak_test()
async def test_home_local_settings_override_repo(yak: Yak) -> None:
    (yak.cwd / ".yaksettings.toml").write_text(
        '[log_download]\nlog_url = "repo_root"\n'
    )
    _write_home_local_settings(yak, '[log_download]\nlog_url = "home_local"\n')

    await yak.server()
    assert await _log_download_method(yak) == {"Curl": "home_local"}


@yak_test()
async def test_repo_local_settings_override_repo_and_home_local(yak: Yak) -> None:
    (yak.cwd / ".yaksettings.toml").write_text(
        '[log_download]\nlog_url = "repo_root"\n'
    )
    _write_home_local_settings(yak, '[log_download]\nlog_url = "home_local"\n')
    (yak.cwd / ".yaksettings.local.toml").write_text(
        '[log_download]\nlog_url = "repo_local"\n'
    )

    await yak.server()
    assert await _log_download_method(yak) == {"Curl": "repo_local"}


@yak_test()
async def test_cli_settings_override_local(yak: Yak) -> None:
    _write_home_local_settings(yak, '[log_download]\nlog_url = "home_local"\n')
    (yak.cwd / ".yaksettings.local.toml").write_text(
        '[log_download]\nlog_url = "repo_local"\n'
    )

    await yak.server(
        "--setting",
        "log_download.log_url=args",
    )
    assert await _log_download_method(yak) == {"Curl": "args"}


@yak_test()
async def test_cli_settings_after_mode(yak: Yak) -> None:
    (yak.cwd / "mode").write_text("--verbose=0\n")

    await yak.server("@mode", "--setting", "log_download.log_url=args")
    assert await _log_download_method(yak) == {"Curl": "args"}
