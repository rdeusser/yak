# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os

from core.common.io.file_watcher import (
    FileWatcherProvider,
    get_file_watcher_events,
)
from core.common.io.file_watcher_tests import (
    git,
    git_commit,
    setup_file_watcher_test,
)
from e2e_util.api.yak import Yak


# Creates this history, with the working copy at commit_d:
#
#   commit_a -- commit_b
#          \
#           -- commit_c -- commit_d
async def setup_file_watcher_scm_test(yak: Yak) -> tuple[str, str, str, str]:
    # Run after setup_file_watcher_test to create a simple stack of commits
    commit_a = git(yak, "rev-parse", "HEAD").strip()

    # Create a file
    path = os.path.join(yak.cwd, "files", "def")
    with open(path, "a"):
        pass

    # Commit it
    commit_b = git_commit(yak, "commit_b")

    # Go back to commit_a
    git(yak, "checkout", "--quiet", commit_a)

    # Create a file
    path = os.path.join(yak.cwd, "files", "ghi")
    with open(path, "a"):
        pass

    # Commit it
    commit_c = git_commit(yak, "commit_c")

    # Create a file
    path = os.path.join(yak.cwd, "files", "jkl")
    with open(path, "a"):
        pass

    # Commit it
    commit_d = git_commit(yak, "commit_d")

    # clear log - run build twice
    await yak.targets("root//:")
    await yak.targets("root//:")

    return commit_a, commit_b, commit_c, commit_d


async def run_checkout_mergebase_changes_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)

    # Create a file
    path = os.path.join(yak.cwd, "files", "def")
    with open(path, "a"):
        pass

    # Commit it
    commit_a = git_commit(yak, "next")

    # Create a file
    path = os.path.join(yak.cwd, "files", "ghi")
    with open(path, "a"):
        pass

    # Commit it
    commit_b = git_commit(yak, "next")

    # Go back to the previous commit
    git(yak, "checkout", "--quiet", commit_a)

    is_fresh_instance, _ = await get_file_watcher_events(yak)
    if file_watcher_provider in [
        FileWatcherProvider.FS_HASH_CRAWLER,
        FileWatcherProvider.RUST_NOTIFY,
    ]:
        # Stats only records the first 100 events (`MAX_FILE_CHANGE_RECORDS` in
        # app/yak_file_watcher/src/stats.rs), so we can't verify the results
        # when making commit transitions
        assert not is_fresh_instance
    else:
        # We might have some events even for a fresh instance, so we ignore
        assert is_fresh_instance

    # Go back to the next commit
    git(yak, "checkout", "--quiet", commit_b)

    is_fresh_instance, results = await get_file_watcher_events(yak)
    print(results)

    if file_watcher_provider in [
        FileWatcherProvider.FS_HASH_CRAWLER,
        FileWatcherProvider.RUST_NOTIFY,
    ]:
        assert not is_fresh_instance
    else:
        assert is_fresh_instance


async def run_checkout_with_mergebase_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    [_, _, commit_c, _] = await setup_file_watcher_scm_test(yak)

    # Go back to commit_c
    git(yak, "checkout", "--quiet", commit_c)

    is_fresh_instance, results = await get_file_watcher_events(yak)
    print(results)

    assert not is_fresh_instance


async def run_rebase_with_mergebase_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    [_, commit_b, commit_c, _] = await setup_file_watcher_scm_test(yak)

    # Rebase C->D from A to B
    git(yak, "rebase", "--quiet", "--onto", commit_b, f"{commit_c}^")

    is_fresh_instance, results = await get_file_watcher_events(yak)
    print(results)

    # Watchman is flaky on native file systems
    if file_watcher_provider != FileWatcherProvider.WATCHMAN:
        assert not is_fresh_instance


async def run_restack_with_mergebase_test(
    yak: Yak,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(yak)
    [commit_a, _, _, commit_d] = await setup_file_watcher_scm_test(yak)

    # Rebase D from C to A
    git(yak, "rebase", "--quiet", "--onto", commit_a, f"{commit_d}^")

    is_fresh_instance, results = await get_file_watcher_events(yak)
    print(results)

    assert not is_fresh_instance
