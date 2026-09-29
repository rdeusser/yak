# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import platform

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env


_YAK_TEST_DECORATOR = yak_test(
    # On windows, we get an error of form
    # "The process cannot access the file because it is being used by another process"
    # when trying to kill the daemon with sqlite states enabled. This is most
    # likely because we don't kill all child processes of the daemon and so the sqlite process
    # is still running and accessing the sqlite db file when being killed. Given this is a
    # pre-existing issue, we disable sqlite state on windows for now.
    extra_yak_config={
        "yak": {
            "sqlite_materializer_state": "false",
            "sqlite_incremental_state": "false",
        },
    }
    if platform.system() == "Windows"
    else {},
)


@_YAK_TEST_DECORATOR
@env("YAK_TEST_FAIL_YAKD_AUTH", "true")
async def test_kill_error(yak: Yak) -> None:
    # Performing a build should fail, since we will not be able to authenticate to the
    # yak daemon
    await expect_failure(yak.build("//:abc"), stderr_regex="injected auth error")

    # Kill should succeed, even though we cannot authenticate to the daemon
    await yak.kill()


@_YAK_TEST_DECORATOR
@env("YAK_TEST_FAIL_YAKD_AUTH", "true")
async def test_clean_error(yak: Yak) -> None:
    # Performing a build should fail, since we will not be able to authenticate to the
    # yak daemon
    await expect_failure(yak.build("//:abc"), stderr_regex="injected auth error")

    # Clean should succeed, even though we cannot authenticate to the daemon
    await yak.clean()
