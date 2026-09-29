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
async def test_bxl_analysis(yak: Yak) -> None:
    result = await yak.bxl(
        "//analysis.bxl:providers_test",
    )

    lines = result.stdout.splitlines()
    assert "provides_foo_foo" in lines[0]
    assert "provides_foo_foo" in lines[1]

    result = await yak.bxl(
        "//analysis.bxl:dependency_test",
    )

    assert result.stdout.splitlines() == [
        "Dependency",
        "root//:stub (<unspecified>)",
    ]


@yak_test(write_invocation_record=True)
async def test_bxl_analysis_missing_subtarget(yak: Yak) -> None:
    res = await expect_failure(
        yak.bxl(
            "//analysis.bxl:missing_subtarget_test",
        ),
        stderr_regex="requested sub target named `missing_subtarget` .* is not available",
    )

    record = res.invocation_record()
    errors = record["errors"]

    assert len(errors) == 1
    assert errors[0]["category"] == "USER"


@yak_test()
async def test_bxl_analysis_unconfigured_target_error(yak: Yak) -> None:
    await expect_failure(
        yak.bxl("//analysis.bxl:unconfigured_target_error_test"),
        stderr_regex="Type of parameter `labels` doesn't match",
    )
