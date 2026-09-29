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
from e2e_util.api.buck import Buck


# Creates this history, with the working copy at commit_d:
#
#   commit_a -- commit_b
#          \
#           -- commit_c -- commit_d
async def setup_file_watcher_scm_test(buck: Buck) -> tuple[str, str, str, str]:
    # Run after setup_file_watcher_test to create a simple stack of commits
    commit_a = git(buck, "rev-parse", "HEAD").strip()

    # Create a file
    path = os.path.join(buck.cwd, "files", "def")
    with open(path, "a"):
        pass

    # Commit it
    commit_b = git_commit(buck, "commit_b")

    # Go back to commit_a
    git(buck, "checkout", "--quiet", commit_a)

    # Create a file
    path = os.path.join(buck.cwd, "files", "ghi")
    with open(path, "a"):
        pass

    # Commit it
    commit_c = git_commit(buck, "commit_c")

    # Create a file
    path = os.path.join(buck.cwd, "files", "jkl")
    with open(path, "a"):
        pass

    # Commit it
    commit_d = git_commit(buck, "commit_d")

    # clear log - run build twice
    await buck.targets("root//:")
    await buck.targets("root//:")

    return commit_a, commit_b, commit_c, commit_d


async def run_checkout_mergebase_changes_test(
    buck: Buck,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(buck)

    # Create a file
    path = os.path.join(buck.cwd, "files", "def")
    with open(path, "a"):
        pass

    # Commit it
    commit_a = git_commit(buck, "next")

    # Create a file
    path = os.path.join(buck.cwd, "files", "ghi")
    with open(path, "a"):
        pass

    # Commit it
    commit_b = git_commit(buck, "next")

    # Go back to the previous commit
    git(buck, "checkout", "--quiet", commit_a)

    is_fresh_instance, _ = await get_file_watcher_events(buck)
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
    git(buck, "checkout", "--quiet", commit_b)

    is_fresh_instance, results = await get_file_watcher_events(buck)
    print(results)

    if file_watcher_provider in [
        FileWatcherProvider.FS_HASH_CRAWLER,
        FileWatcherProvider.RUST_NOTIFY,
    ]:
        assert not is_fresh_instance
    else:
        assert is_fresh_instance


async def run_checkout_with_mergebase_test(
    buck: Buck,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(buck)
    [_, _, commit_c, _] = await setup_file_watcher_scm_test(buck)

    # Go back to commit_c
    git(buck, "checkout", "--quiet", commit_c)

    is_fresh_instance, results = await get_file_watcher_events(buck)
    print(results)

    assert not is_fresh_instance


async def run_rebase_with_mergebase_test(
    buck: Buck,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(buck)
    [_, commit_b, commit_c, _] = await setup_file_watcher_scm_test(buck)

    # Rebase C->D from A to B
    git(buck, "rebase", "--quiet", "--onto", commit_b, f"{commit_c}^")

    is_fresh_instance, results = await get_file_watcher_events(buck)
    print(results)

    # Watchman is flaky on native file systems
    if file_watcher_provider != FileWatcherProvider.WATCHMAN:
        assert not is_fresh_instance


async def run_restack_with_mergebase_test(
    buck: Buck,
    file_watcher_provider: FileWatcherProvider,
) -> None:
    await setup_file_watcher_test(buck)
    [commit_a, _, _, commit_d] = await setup_file_watcher_scm_test(buck)

    # Rebase D from C to A
    git(buck, "rebase", "--quiet", "--onto", commit_a, f"{commit_d}^")

    is_fresh_instance, results = await get_file_watcher_events(buck)
    print(results)

    assert not is_fresh_instance
