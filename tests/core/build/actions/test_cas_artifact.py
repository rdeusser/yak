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
async def test_cas_artifact_expiration_out_of_range(yak: Yak) -> None:
    # Regression test: this used to panic the daemon instead of reporting an error.
    await expect_failure(
        yak.build("root//:out_of_range_expiration"),
        stderr_regex="Out-of-range value `4611686018427387904` for expires_after_timestamp",
    )
