# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import tempfile

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    ["build", "targets", "cquery", "bxl", "uquery"],
)
async def test_write_uuid(yak: Yak, cmd: str) -> None:
    with tempfile.NamedTemporaryFile() as file:
        cmd_call = getattr(yak, cmd)
        await expect_failure(cmd_call("--write-build-id", file.name, "a"))

        assert len(file.read()) > 0


@yak_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    ["build", "targets", "cquery", "bxl", "uquery"],
)
async def test_ban_cell_override(yak: Yak, cmd: str) -> None:
    cmd_call = getattr(yak, cmd)
    await expect_failure(cmd_call("--config", "repositories.foo=bar", "a"))
    await expect_failure(cmd_call("--config", "cells.foo=bar", "a"))
