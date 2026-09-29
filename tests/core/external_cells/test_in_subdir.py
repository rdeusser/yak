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
async def test_in_subdir(yak: Yak) -> None:
    err = "No such file or directory"
    await expect_failure(
        yak.targets("test_bundled_cell//dir:"),
        stderr_regex=err,
    )
    await expect_failure(
        yak.cquery("root//:"),
        stderr_regex=err,
    )
    # FIXME(JakobDegen): Decide if this is a bug or not
    (yak.cwd / "somedir").mkdir()
    await yak.targets("test_bundled_cell//dir:")
    await yak.cquery("root//:")
