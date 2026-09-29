# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_relative_path_basic(yak: Yak) -> None:
    assert "//foo/bar:test_basic" in (await yak.targets("//foo/bar:")).stdout


@yak_test()
async def test_relative_path_left_allowed_dir(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//foo/baz:"),
        stderr_regex="Relative import path `../../defs.bzl` is not allowed at the current location.",
    )


@yak_test()
async def test_relative_path_has_symlink(yak: Yak) -> None:
    os.symlink(yak.cwd, os.path.join(yak.cwd, "foo/sym"), target_is_directory=True)
    await expect_failure(
        yak.targets("//foo/sym/foo/bar:"),
        stderr_regex="Symlink found on the way from current dir `root//foo/sym/foo/bar` to allowed relative dir `root//foo`: `root//foo/sym`.",
    )


@yak_test()
async def test_relative_path_in_attribute_default_current(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//foo/default_current:target"),
        stderr_regex="Target pattern must be absolute",
    )


@yak_test()
async def test_relative_path_in_attribute_default_up(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//foo/default_up:target"),
        stderr_regex="Target pattern must be absolute",
    )
