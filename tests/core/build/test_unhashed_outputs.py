# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import shutil
from datetime import datetime, timedelta
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_unhashed_putputs(yak: Yak) -> None:
    await yak.build("//pack:trivial_build")

    p = yak.cwd / "yak-out" / "v2" / "gen" / "root" / "pack" / "foo.txt"
    assert p.exists()
    assert p.is_symlink()


@yak_test()
async def test_projected_output(yak: Yak) -> None:
    await yak.build("//:projected_output")

    p = yak.cwd / "yak-out" / "v2" / "gen" / "root" / "dir"
    assert p.exists()
    assert p.is_symlink()
    assert (p / "file").is_file()


@yak_test()
async def test_materializer_managed_unhashed_output(yak: Yak) -> None:
    await yak.build("//pack:trivial_build")

    p = yak.cwd / "yak-out" / "v2" / "gen" / "root" / "pack" / "foo.txt"
    assert p.is_symlink()

    materializer_state = await yak.audit("deferred-materializer", "list")
    assert "yak-out/v2/gen/root/pack/foo.txt" in materializer_state.stdout

    future_time = int((datetime.now() + timedelta(weeks=7)).timestamp())
    await yak.clean(f"--keep-since-time={future_time}")
    assert p.is_symlink()
    assert p.is_file()
    materializer_state = await yak.audit("deferred-materializer", "list")
    assert "yak-out/v2/gen/root/pack/foo.txt" in materializer_state.stdout


@yak_test()
async def test_materializer_managed_unhashed_output_without_materialization(
    yak: Yak,
) -> None:
    await yak.build("//pack:trivial_build", "--materializations=none")
    unhashed = yak.cwd / "yak-out" / "v2" / "gen" / "root" / "pack" / "foo.txt"

    assert not unhashed.is_symlink()

    materializer_state = await yak.audit("deferred-materializer", "list")
    assert "yak-out/v2/gen/root/pack/foo.txt" in materializer_state.stdout


@yak_test()
async def test_build_symlink_does_not_traverse_existing_symlinks(yak: Yak) -> None:
    await yak.build("//pack:trivial_build")
    symlink_folder = yak.cwd / "yak-out" / "v2" / "gen" / "root" / "pack"

    # Now, overwrite part of the symlink path with something we cannot traverse.
    path = symlink_folder.parent
    shutil.rmtree(path)
    # On Windows this is just non existing path.
    os.symlink("/dev/null", path)

    # Can we still build? If we delete the symlink when walking up the path, we
    # can. If we traverse it, we can't.
    await yak.build("//pack:trivial_build")


@yak_test()
async def test_conflict_with_content_based_paths(yak: Yak) -> None:
    symlink_path: Path = (
        yak.cwd / "yak-out" / "v2" / "gen" / "root" / "conflict" / "shared_name"
    )
    content_based_path: Path = (
        yak.cwd / "yak-out" / "v2" / "art" / "root" / "conflict" / "shared_name"
    )
    subtarget_output: Path
    # sanity check that we're starting from a clean state
    assert not symlink_path.exists()
    assert not content_based_path.exists()

    def base_checks(*, should_symlink_exist: bool) -> None:
        if should_symlink_exist:
            assert symlink_path.is_symlink()
            assert symlink_path.resolve().is_file()
        else:
            assert not symlink_path.exists()

        assert content_based_path.is_dir()
        assert subtarget_output.is_symlink()
        assert subtarget_output.resolve().is_file()
        assert not subtarget_output.resolve().is_relative_to(symlink_path)
        assert subtarget_output.resolve().is_relative_to(content_based_path)
        # Verify we can read the contents of the file
        with open(subtarget_output) as f:
            f.read()

    #
    # Build just the subtarget. Esnsure that the subtarget output exists and is
    # reacable, and that it lives in the place we expect.
    #
    res = await yak.build(
        "//conflict/shared_name:subtarget",
        "--config",
        "yak.create_unhashed_links=false",
    )
    subtarget_output = res.get_build_report().output_for_target(
        "root//conflict/shared_name:subtarget"
    )
    base_checks(should_symlink_exist=False)

    #
    # Build the conflicting target w/o unhashed links. This should leave the
    # subtarget_output alone, which should remain readable.
    #
    await yak.build(
        "//conflict:shared_name",
        "--config",
        "yak.create_unhashed_links=false",
    )
    # TODO(jtbraun): this should instead ensure the symlink does NOT exist, and the content_based_path does and is a folder
    base_checks(should_symlink_exist=False)

    #
    # Build the conflicting target with unhashed links. This will overwrite the
    # subtarget with a directory, and the symlink_path will now exist.
    #
    await yak.build("//conflict:shared_name")
    base_checks(should_symlink_exist=True)


@yak_test()
async def test_projected_symlink_output(yak: Yak) -> None:
    result = await yak.build("//:projected_symlink_output")
    output = result.get_build_report().output_for_target(
        "root//:projected_symlink_output"
    )

    assert output.parent.is_symlink()
    assert output.is_symlink()
    assert output.resolve().is_file()
