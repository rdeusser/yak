# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import shutil

import pytest
from core.common.io.file_watcher_tests import run_aba_test
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

# The daemon finds the Watchman server by running the `watchman` program on
# PATH.
pytestmark = pytest.mark.skipif(
    shutil.which("watchman") is None, reason="needs Watchman"
)


@yak_test()
async def test_watchman_aba(yak: Yak) -> None:
    await run_aba_test(yak)
