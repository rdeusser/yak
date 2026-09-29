# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_lint_fails(yak: Yak) -> None:
    await expect_failure(
        yak.starlark("lint", "bad_warning.bzl"),
        stderr_regex="Found 3 lints",
    )


@yak_test()
async def test_typecheck_fails(yak: Yak) -> None:
    await yak.starlark("typecheck", "good.bzl")
    await expect_failure(
        yak.starlark("typecheck", "bad.bzl"),
        stderr_regex="Detected 2 errors",
    )
