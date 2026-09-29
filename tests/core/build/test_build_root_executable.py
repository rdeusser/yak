# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

"""
Test that when we render paths relative to the repo root, we prefix them with a
`./` to ensure the OS executes the cwd-relative path and doesn't do a $PATH
lookup for them.
"""


@yak_test()
async def test_build_root_executable_local(yak: Yak) -> None:
    await yak.build(":top", "--local-only")


@pytest.mark.remote_execution
@yak_test()
async def test_build_root_executable_remote(yak: Yak) -> None:
    await yak.build(":top", "--remote-only")
