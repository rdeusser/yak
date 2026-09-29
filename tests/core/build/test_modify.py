# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import fileinput
import os
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import random_string


@yak_test(data_dir="modify")
async def test_modify_genrule(yak: Yak) -> None:
    result = await yak.build("//:writer")
    output = result.get_build_report().output_for_target("root//:writer")
    assert Path(output).read_text() == "HELLO\n"

    # Change "HELLO" in YAK.fixture to "GOODBYE"
    with fileinput.input(yak.cwd / "YAK.fixture", inplace=True) as f:
        for line in f:
            print(line.replace("HELLO", "GOODBYE"), end="")

    result = await yak.build("//:writer")
    output = result.get_build_report().output_for_target("root//:writer")
    assert Path(output).read_text() == "GOODBYE\n"


@yak_test(data_dir="modify")
async def test_modify_src(yak: Yak) -> None:
    result = await yak.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    assert Path(output).read_text() == "HELLO\n"

    (yak.cwd / "src.txt").write_text("GOODBYE\n")
    result = await yak.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    assert Path(output).read_text() == "GOODBYE\n"


@yak_test(data_dir="modify")
async def test_modify_genrule_notify(yak: Yak) -> None:
    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("\n[yak]\nfile_watcher = notify")
    await yak.kill()  # Ensure the config gets picked up
    await test_modify_genrule(yak)


@yak_test(data_dir="modify")
async def test_modify_directory(yak: Yak) -> None:
    # Checks that a build notices a directory that was deleted along with its file.
    os.mkdir(yak.cwd / "a_dir")
    with open(yak.cwd / "a_dir" / "test.txt", "w") as file:
        file.write("test")
    await yak.build("//:writer")
    # Remove a directory, and change a file, so the file gets spotted,
    # and we'd better note that the directory no longer exists
    os.remove(yak.cwd / "a_dir" / "test.txt")
    os.rmdir(yak.cwd / "a_dir")
    await yak.build("//:writer")


@pytest.mark.remote_execution
@yak_test(data_dir="modify_file_during_build")
async def test_modify_file_during_build(yak: Yak) -> None:
    # We need to write some random stuff to the file first so that yak will
    # have to attempt to upload it to RE (which will fail because by that time
    # we will have overwritten it with other content).
    with open(yak.cwd / "text", "w", encoding="utf-8") as f:
        f.write(random_string())

    await expect_failure(
        yak.build("//:check"),
        stderr_regex="modified files while the build was in progress",
    )


@pytest.mark.remote_execution
@yak_test(data_dir="modify_file_during_build")
async def test_file_notify(yak: Yak) -> None:
    # We need to write some random stuff to the file first so that yak will
    # have to attempt to upload it to RE (which will fail because by that time
    # we will have overwritten it with other content).
    with open(yak.cwd / "text", "w", encoding="utf-8") as f:
        f.write(random_string())

    await expect_failure(
        yak.build("//:check"),
        stderr_regex="modified files while the build was in progress",
    )
