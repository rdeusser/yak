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
from e2e_util.asserts import expect_failure


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


async def run_rename_parent_directory_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    """The operating system reports a renamed directory as one event, but a query of the package
    `files/d` reads the listing of `files/d` and its build file directly."""
    await setup_file_watcher_test(yak)
    with open(os.path.join(yak.cwd, "files", "d", "YAK.fixture"), "w") as f:
        f.write('print("Package d")\n')
    await yak.targets("root//files/d:")

    os.rename(os.path.join(yak.cwd, "files"), os.path.join(yak.cwd, "other"))
    await expect_failure(
        yak.targets("root//files/d:"),
        stderr_regex="files/d",
    )
    await yak.targets("root//other/d:")

    os.rename(os.path.join(yak.cwd, "other"), os.path.join(yak.cwd, "files"))
    await yak.targets("root//files/d:")
