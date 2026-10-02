# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import shutil
from pathlib import Path

import pytest
from core.common.io.file_watcher import FileWatcherProvider
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.helper.utils import filter_events
from e2e_util.yak_workspace import yak_test


async def file_watcher_provider(yak: Yak, env: dict[str, str]) -> FileWatcherProvider:
    await yak.targets("root//:", env=env)
    providers = await filter_events(
        yak, "Event", "data", "SpanStart", "data", "FileWatcher", "provider"
    )
    assert len(set(providers)) == 1, providers
    return FileWatcherProvider(providers[0])


@pytest.mark.skipif(shutil.which("watchman") is None, reason="needs Watchman")
@yak_test()
async def test_auto_selects_watchman_when_installed(yak: Yak) -> None:
    assert await file_watcher_provider(yak, {}) == FileWatcherProvider.WATCHMAN


@pytest.mark.skipif(
    "WATCHMAN_SOCK" in os.environ, reason="WATCHMAN_SOCK selects Watchman"
)
@yak_test()
async def test_auto_selects_notify_without_watchman(yak: Yak) -> None:
    path = os.pathsep.join(
        entry
        for entry in os.environ["PATH"].split(os.pathsep)
        if shutil.which("watchman", path=entry) is None
    )
    assert shutil.which("watchman", path=path) is None
    provider = await file_watcher_provider(yak, {"PATH": path})
    assert provider == FileWatcherProvider.RUST_NOTIFY


@yak_test()
async def test_auto_watchman_failure_names_notify(yak: Yak, tmp_path: Path) -> None:
    await expect_failure(
        yak.targets("root//:", env={"WATCHMAN_SOCK": str(tmp_path / "missing.sock")}),
        stderr_regex="Set `yak.file_watcher = notify`",
    )
