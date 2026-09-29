# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


async def _build_output(yak: Yak, target: str) -> Path:
    result = await yak.build(target, "--show-output")
    path = result.get_target_to_build_output().get(target)
    assert path is not None
    return yak.cwd / path


# Symlink materialization assertions are unreliable on Windows.
@yak_test(skip_for_os=["windows"])
async def test_assembled_dir_mixes_copies_and_symlinks(yak: Yak) -> None:
    out = await _build_output(yak, "root//:mixed")
    assert out.is_dir()

    # `assembled_dir.copy` entries are materialized as real files, with the
    # source's bytes, at their entry paths (including nested ones).
    for name, data in [
        ("bin/exe", "exe-bytes"),
        ("bin/exe.resources.json", "manifest-bytes"),
        ("src_copy.txt", "source-file-bytes\n"),
    ]:
        entry = out / name
        assert entry.is_file(), name
        assert not entry.is_symlink(), name
        assert entry.read_text() == data, name

    # `assembled_dir.symlink` entries are materialized as symlinks that
    # resolve to the source artifact's bytes.
    for name, data in [
        ("deep/nested/link", "exe-bytes"),
        ("src_link.txt", "source-file-bytes\n"),
    ]:
        entry = out / name
        assert entry.is_symlink(), name
        assert entry.read_text() == data, name

    # A symlinked directory artifact resolves as a directory.
    res_dir = out / "res"
    assert res_dir.is_symlink()
    assert (res_dir / "data.txt").read_text() == "resource-bytes"


@yak_test(skip_for_os=["windows"])
async def test_assembled_dir_is_a_usable_input(yak: Yak) -> None:
    # A downstream action can read both copied and symlinked entries through
    # the assembled dir (i.e. the entries' sources are tracked as inputs).
    out = await _build_output(yak, "root//:consumed")
    assert out.read_text() == "exe-bytes|source-file-bytes\n"


@yak_test()
async def test_assembled_dir_rejects_overlapping_paths(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:overlap_fail"),
        stderr_regex="must be non-overlapping",
    )


@yak_test()
async def test_assembled_dir_rejects_empty_path(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:empty_path_fail"),
        stderr_regex="must not be empty",
    )


@yak_test()
async def test_assembled_dir_rejects_untyped_entries(yak: Yak) -> None:
    # A bare artifact is not a valid entry: contents values must be built
    # with `assembled_dir.copy(...)` / `assembled_dir.symlink(...)`.
    await expect_failure(
        yak.build("root//:untyped_entry_fail"),
        stderr_regex="AssembledDirEntry",
    )
