# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_cell_ordering(yak: Yak) -> None:
    res = await yak.audit("cell")
    # The repository should be in the list, not the alias
    assert "source:" in res.stdout
    assert "a:" not in res.stdout
    assert "z:" not in res.stdout

    res = await yak.audit("cell", "--aliases")
    assert "source:" in res.stdout
    assert "a:" in res.stdout
    assert "z:" in res.stdout


@yak_test()
async def test_bxl_audit_cell(yak: Yak) -> None:
    result = await yak.bxl("//test_audit.bxl:audit_cell")

    # specify single cell
    outputs = result.stdout.splitlines()
    single_result = json.loads(outputs[0])
    assert single_result["source"] == str(yak.cwd / "fbs")

    # don't specify cell - should return all cell aliases
    all_result = json.loads(outputs[1])
    assert all_result["a"] == str(yak.cwd / "fbs")
    assert all_result["z"] == str(yak.cwd / "fbc")
    assert all_result["code"] == str(yak.cwd / "fbc")
    assert all_result["source"] == str(yak.cwd / "fbs")
