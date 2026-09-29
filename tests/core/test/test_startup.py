# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import sys

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env


@yak_test()
@env("YAK_TEST_EXECUTOR_USE_TCP", "true")
async def test_tcp_startup_fail(yak: Yak) -> None:
    # Python is a binary that will just fail when we give it our executor args
    # but works on any platform. It's a bit dumb but it'll do
    await expect_failure(
        yak.test("...", test_executor=sys.executable),
        stderr_regex="Executor exited before connecting",
    )
