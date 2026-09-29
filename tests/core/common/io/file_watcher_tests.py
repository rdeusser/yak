# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import subprocess

from core.common.io.file_watcher import FileWatcherEvent
from core.common.io.utils import get_files
from e2e_util.api.buck import Buck


def git(buck: Buck, *args: str) -> str:
    """Runs git on the repository in the test project and returns its stdout.

    `--git-dir` pins git to that repository, and the user's git configuration
    is ignored so that hooks and signing settings do not apply.
    """
    project = str(buck.cwd)
    result = subprocess.run(
        [
            "git",
            f"--git-dir={os.path.join(project, '.git')}",
            f"--work-tree={project}",
            "-c",
            "user.name=yak tests",
            "-c",
            "user.email=tests@example.com",
            *args,
        ],
        cwd=project,
        env={
            **os.environ,
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
        },
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return result.stdout


def git_commit(buck: Buck, message: str) -> str:
    """Commits every change in the test project and returns the new commit."""
    git(buck, "add", "--all")
    git(buck, "commit", "--quiet", "--message", message)
    return git(buck, "rev-parse", "HEAD").strip()


async def setup_file_watcher_test(buck: Buck) -> None:
    git(buck, "init", "--quiet", "--initial-branch=main")
    (buck.cwd / ".gitignore").write_text("/yak-out\n")
    git_commit(buck, "temp")

    status = git(buck, "status", "--porcelain")
    assert status == "", (
        f"Expected clean working directory, but `git status` returned:\n{status}"
    )
    assert (await get_files(buck)) == ["files/abc", "files/d/empty"]


def verify_results(
    results: list[FileWatcherEvent],
    required: list[FileWatcherEvent],
) -> None:
    for req in required:
        if req not in results:
            print(f"results={results}")
            print(f"required={required}")
            assert req in results, "required not in results"


async def run_aba_test(buck: Buck) -> None:
    await setup_file_watcher_test(buck)

    git(buck, "mv", "files/abc", "files/d/")
    assert (await get_files(buck)) == ["files/d/abc", "files/d/empty"]

    # Sets the move aside, which restores the committed files.
    git(buck, "stash", "--quiet")
    assert (await get_files(buck)) == ["files/abc", "files/d/empty"]
