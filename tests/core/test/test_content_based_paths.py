# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_run_test_with_content_based_path(yak: Yak) -> None:
    await yak.test("root//:run_test_with_content_based_path")


@yak_test()
async def test_platform_resolution(yak: Yak) -> None:
    await yak.test(
        ":local_resources_test",
        test_executor="",
    )
    res = await yak.log("what-ran")
    assert "MY_RESOURCE_ID=42" in res.stdout
