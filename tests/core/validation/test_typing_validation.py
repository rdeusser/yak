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
async def test_typing_validation_fails_on_type_errors(yak: Yak) -> None:
    await expect_failure(
        yak.build(":bad_types_validated"),
        stderr_regex="Validation for .+ failed.*is not assignable to parameter",
    )


@yak_test()
async def test_typing_validation_passes_on_clean_types(yak: Yak) -> None:
    await yak.build(":good_types_validated")


@yak_test()
async def test_typing_validation_off_ignores_errors(yak: Yak) -> None:
    await yak.build(":bad_types_no_validation")


@yak_test()
async def test_typing_disabled_skips_validation(yak: Yak) -> None:
    await yak.build(":typing_disabled")
