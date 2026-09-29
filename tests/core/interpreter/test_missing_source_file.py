# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env


@yak_test()
@env(
    "YAK_HARD_ERROR",
    "true",
)
async def test_missing_source_file_when_hard_errors_enabled(yak: Yak) -> None:
    await expect_failure(
        yak.uquery("//package1:"),
        stderr_regex="Source file `non_existent_source_file.txt` does not exist as a member of package `prelude//package1`",
    )


@yak_test()
@env(
    "YAK_HARD_ERROR",
    "false",
)
async def test_missing_source_file_when_hard_errors_disabled(yak: Yak) -> None:
    # `source_file_missing` is a hard error, so the command fails even when
    # `YAK_HARD_ERROR` turns other soft errors into warnings.
    await expect_failure(
        yak.uquery("//package1:"),
        stderr_regex="Source file `non_existent_source_file.txt` does not exist as a member of package `prelude//package1`",
    )
