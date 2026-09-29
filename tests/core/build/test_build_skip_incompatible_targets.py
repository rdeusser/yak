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
async def test_build_skip_incompatible(yak: Yak) -> None:
    targetA = "root//:compatible-with-A"
    targetB = "root//:compatible-with-B"
    platformA = "root//:platA"

    await expect_failure(
        yak.build(
            targetA,
            targetB,
            f"--target-platforms={platformA}",
        ),
        stderr_regex=rf"{targetB}\s*is incompatible with {platformA}",
    )

    result = await yak.build(
        targetA,
        targetB,
        f"--target-platforms={platformA}",
        "--skip-incompatible-targets",
    )
    assert "Skipped 1 incompatible target" in result.stderr
    assert targetB in result.stderr

    report = result.get_build_report()
    assert len(report.results) == 2
    assert len(report.results[targetA]["configured"]) == 1
    assert len(report.results[targetB]["configured"]) == 0
