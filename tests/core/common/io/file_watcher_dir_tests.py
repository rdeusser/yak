# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import shutil

from core.common.io.file_watcher import (
    FileWatcherEvent,
    FileWatcherEventType,
    FileWatcherKind,
    FileWatcherProvider,
    get_file_watcher_events,
)
from core.common.io.file_watcher_tests import (
    setup_file_watcher_test,
    verify_results,
)
from e2e_util.api.yak import Yak


async def run_create_directory_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    path = os.path.join(yak.cwd, "files", "def")
    os.mkdir(path)

    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE,
            FileWatcherKind.DIRECTORY,
            "root//files/def",
        )
    ]

    is_fresh_instance, results = await get_file_watcher_events(yak)
    assert not is_fresh_instance
    verify_results(results, required)


async def run_remove_directory_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    path = os.path.join(yak.cwd, "files", "d")
    shutil.rmtree(path)

    required = [
        FileWatcherEvent(
            FileWatcherEventType.DELETE,
            FileWatcherKind.DIRECTORY,
            "root//files/d",
        ),
    ]

    is_fresh_instance, results = await get_file_watcher_events(yak)
    assert not is_fresh_instance
    verify_results(results, required)


async def run_rename_directory_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    fromPath = os.path.join(yak.cwd, "files", "d")
    toPath = os.path.join(yak.cwd, "files", "def")
    os.rename(fromPath, toPath)

    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE,
            FileWatcherKind.DIRECTORY,
            "root//files/def",
        ),
        FileWatcherEvent(
            FileWatcherEventType.DELETE,
            FileWatcherKind.DIRECTORY,
            "root//files/d",
        ),
    ]

    is_fresh_instance, results = await get_file_watcher_events(yak)
    assert not is_fresh_instance
    verify_results(results, required)
