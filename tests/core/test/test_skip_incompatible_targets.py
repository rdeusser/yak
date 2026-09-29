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
async def test_test_skip_incompatible_targets(yak: Yak) -> None:
    targetA = "root//:compatible-with-A"
    targetB = "root//:compatible-with-B"
    platformA = "root//:platA"

    await expect_failure(
        yak.test(
            targetA,
            targetB,
            f"--target-platforms={platformA}",
            test_executor="",
        ),
        stderr_regex=rf"{targetB}\s*is incompatible with {platformA}#.*$",
    )

    result = await yak.test(
        targetA,
        targetB,
        f"--target-platforms={platformA}",
        "--skip-incompatible-targets",
        test_executor="",
    )
    assert targetA in result.stderr
    assert targetB not in result.stderr

    result.check_returncode()
