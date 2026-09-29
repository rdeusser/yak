# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import random
import string

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_validation_concurrent(yak: Yak) -> None:
    # There are 2 actions — slow build action and fast validation action.
    # Check that validation doesn't wait for a slow DefaultInfo artifact to be built and fails the build first.
    await expect_failure(
        yak.build(
            ":plate",
            "-c",
            f"test.cache_buster={_random_string()}",
        ),
        stderr_regex="Validation for `.+` failed",
    )


def _random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for _ in range(256))
