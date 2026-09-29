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
async def test_build_artifact(yak: Yak) -> None:
    res = await yak.bxl(
        "//:lazy_build_artifact.bxl:build_artifact",
    )
    assert "foo.txt" in res.stdout
    assert "bar.txt" in res.stdout


@yak_test()
async def test_build_artifact_catch_error(yak: Yak) -> None:
    res = await yak.bxl(
        "//:lazy_build_artifact.bxl:build_artifact_fail",
    )
    assert "foo.txt" in res.stdout


@yak_test()
async def test_cannot_build_dynmiac_action_output(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(
            "//:lazy_build_artifact.bxl:dynamic",
        ),
        stderr_regex="does not accept declared artifact",
    )


@yak_test()
async def test_cannot_bxl_action_output(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(
            "//:lazy_build_artifact.bxl:bxl_action_output",
        ),
        stderr_regex="does not accept declared artifact",
    )
