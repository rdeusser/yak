# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import shutil
import tempfile
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import expect_exec_count


def setup_symlink(symlink_path: Path, target: Path) -> None:
    symlink_path.parent.mkdir(parents=True, exist_ok=True)

    if not os.path.islink(symlink_path) and os.path.isdir(symlink_path):
        shutil.rmtree(symlink_path)
    else:
        symlink_path.unlink(missing_ok=True)

    os.symlink(target, symlink_path)


@yak_test(extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}})
async def test_symlink_target_tracked_for_rebuild(yak: Yak) -> None:
    setup_symlink(yak.cwd / "src" / "link", Path("../dir"))

    await yak.build("//:cp")
    await expect_exec_count(yak, 1)

    await yak.build("//:cp")
    await expect_exec_count(yak, 0)

    with open(yak.cwd / "dir/file", "w") as file:
        file.write("GOODBYE\n")

    # This isn't really behavior  we want to guarantee and we'd rather users
    # don't use symlinks, but this is very observable (and it's not worse than
    # just reading the files then pretending they are never used!)
    await yak.build("//:cp")
    await expect_exec_count(yak, 1)


@pytest.mark.xfail(
    reason="the fs_hash_crawler file watcher that tests use does not notice a symlink changing its target",
    strict=True,
)
@yak_test(
    extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}},
)
async def test_symlinks_redirection(yak: Yak) -> None:
    setup_symlink(yak.cwd / "src" / "link", Path("../dir"))

    await yak.build("//:cp")
    await expect_exec_count(yak, 1)

    await yak.build("//:cp")
    await expect_exec_count(yak, 0)

    # We change the symlink which should invalidate all files depending on it
    setup_symlink(yak.cwd / "src" / "link", Path("../dir2"))

    await yak.build("//:cp")
    await expect_exec_count(yak, 1)


@pytest.mark.xfail(
    reason="the fs_hash_crawler file watcher that tests use does not notice a symlink changing its target",
    strict=True,
)
@yak_test(
    extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}},
)
async def test_symlinks_external(yak: Yak) -> None:
    top_level = Path(tempfile.mkdtemp())

    (top_level / "nested1").mkdir()
    (top_level / "nested2").mkdir()
    (top_level / "nested1" / "file").write_text("HELLO")
    (top_level / "nested2" / "file").write_text("GOODBYE")

    setup_symlink(yak.cwd / "ext" / "link", top_level / "nested1")

    await yak.build("//:ext")
    await expect_exec_count(yak, 1)

    await yak.build("//:ext")
    await expect_exec_count(yak, 0)

    setup_symlink(yak.cwd / "ext" / "link", top_level / "nested2")

    await yak.build("//:ext")
    await expect_exec_count(yak, 1)


@pytest.mark.remote_execution
@yak_test(extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_symlinks(yak: Yak) -> None:
    res = await yak.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    # Just check that we don't always return `True`
    assert res.stdout.strip() == "False"

    setup_symlink(yak.cwd / "src" / "link", Path("..") / "dir")

    res = await yak.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"

    res = await yak.build_without_report(
        "//:stat_symlink_in_dir",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"


@pytest.mark.remote_execution
@yak_test(extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_source_symlinks_to_file(yak: Yak) -> None:
    res = await yak.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    # Just check that we don't always return `True`
    assert res.stdout.strip() == "False"

    setup_symlink(
        yak.cwd / "src" / "link",
        Path("..") / "dir" / "file",
    )

    res = await yak.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"


@yak_test(extra_yak_config={"yak": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_source_symlinks_to_in_symlink_target(yak: Yak) -> None:
    for s in ("dir", "dir2/dir"):
        (yak.cwd / s).mkdir(parents=True, exist_ok=True)
        (yak.cwd / s / "file").write_text(s)
    setup_symlink(yak.cwd / "redirectvia", Path("dir2") / "dir")

    setup_symlink(
        yak.cwd / "src" / "link",
        Path("..") / "redirectvia" / ".." / "dir" / "file",
    )

    res = await yak.build_without_report(
        "//:cp_src_link_via_builtin",
        "--out",
        "-",
    )
    # FIXME(JakobDegen): Should be `dir2/dir`. The fact that `redirectvia`, found in the symlink
    # target, is itself a symlink is completely ignored
    assert res.stdout.strip() == "dir"


@yak_test()
async def test_read_symlink_dir_build_target(yak: Yak) -> None:
    setup_symlink(yak.cwd / "testlink", yak.cwd / "symdir" / "dir")

    await yak.build("//:symlink_dep")


@yak_test()
async def test_read_symlink_dir_list_target(yak: Yak) -> None:
    setup_symlink(yak.cwd / "testlink", yak.cwd / "symdir")

    await yak.targets("//testlink/dir:")
