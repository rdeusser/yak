# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from core.common.io.file_watcher import FileWatcherProvider
from core.common.io.file_watcher_dir_tests import (
    run_create_directory_test,
    run_remove_directory_test,
    run_rename_directory_test,
)
from core.common.io.file_watcher_file_tests import (
    run_create_file_test,
    run_modify_file_test,
    run_remove_file_test,
    run_rename_file_test,
    run_replace_file_test,
)
from core.common.io.file_watcher_scm_tests import (
    run_checkout_mergebase_changes_test,
    run_checkout_with_mergebase_test,
    run_rebase_with_mergebase_test,
    run_restack_with_mergebase_test,
)
from core.common.io.file_watcher_symlink_tests import (
    run_change_symlink_target_test,
    run_create_symlink_test,
    run_replace_file_with_symlink_test,
)
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_notify_create_file(yak: Yak) -> None:
    await run_create_file_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_modify_file(yak: Yak) -> None:
    await run_modify_file_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_remove_file(yak: Yak) -> None:
    await run_remove_file_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_rename_file(yak: Yak) -> None:
    await run_rename_file_test(yak, FileWatcherProvider.RUST_NOTIFY)


# File replace is not supported on Windows
@yak_test(skip_for_os=["windows"])
async def test_notify_replace_file(yak: Yak) -> None:
    await run_replace_file_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_create_directory(yak: Yak) -> None:
    await run_create_directory_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_remove_directory(yak: Yak) -> None:
    await run_remove_directory_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_rename_directory(yak: Yak) -> None:
    await run_rename_directory_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_checkout_mergebase_changes(yak: Yak) -> None:
    await run_checkout_mergebase_changes_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_checkout_with_mergebase(yak: Yak) -> None:
    await run_checkout_with_mergebase_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_rebase_with_mergebase(yak: Yak) -> None:
    await run_rebase_with_mergebase_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_restack_with_mergebase(yak: Yak) -> None:
    await run_restack_with_mergebase_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_create_symlink_test(yak: Yak) -> None:
    await run_create_symlink_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_replace_file_with_symlink_test(yak: Yak) -> None:
    await run_replace_file_with_symlink_test(yak, FileWatcherProvider.RUST_NOTIFY)


@yak_test()
async def test_notify_change_symlink_target_test(yak: Yak) -> None:
    await run_change_symlink_target_test(yak, FileWatcherProvider.RUST_NOTIFY)
