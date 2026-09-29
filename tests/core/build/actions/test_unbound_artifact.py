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
async def test_unbound_artifact(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:action_with_unbound_artifact"),
        stderr_regex="error: Artifact must be bound by now",
    )


@yak_test()
async def test_unbound_artifact_inside_tset(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:action_with_unbound_artifact"),
        stderr_regex="error: Artifact must be bound by now",
    )
