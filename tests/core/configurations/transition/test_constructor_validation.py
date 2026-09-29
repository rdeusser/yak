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
async def test_construction_validation_good(yak: Yak) -> None:
    await yak.targets("//good:")


@yak_test()
async def test_construction_validation_bad(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//bad:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )


@yak_test()
async def test_construction_validation_bad_param_types(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//bad_param_types:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )


@yak_test()
async def test_construction_validation_bad_param_types_vnew(yak: Yak) -> None:
    # FIXME(JakobDegen): Evaluate whether we can implement this. The performance
    # concerns are a bit higher here because the code is hotter.
    await yak.build("//bad_param_types_vnew:")


@yak_test()
async def test_construction_validation_bad_return_type(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//bad_return_type:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )
