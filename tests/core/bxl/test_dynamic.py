# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test()
async def test_bxl_dynamic_action(yak: Yak) -> None:
    result = await yak.bxl(
        "//:dynamic.bxl:dynamic_test",
    )
    outputs = result.stdout.strip()
    assert Path(outputs).read_text() == "content"


@yak_test()
async def test_bxl_dynamic_with_bxl_ctx(yak: Yak) -> None:
    result = await yak.bxl(
        "//:dynamic.bxl:dynamic_test_with_bxl_ctx",
    )

    outputs = json.loads(result.stdout)
    golden_result = {}
    for k, v in outputs.items():
        golden_result.update({k: _replace_hash(Path(v).read_text())})

    golden(
        output=json.dumps(golden_result, indent=2),
        rel_path="happy_path_dynamic_ctx.golden.json",
    )


# A dynamic action cannot read the exec_deps or toolchains of the bxl_ctx that
# created it (`bxl_acessing_exec_platform` is a hard error).
@yak_test()
async def test_bxl_dynamic_execution_resolution(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(
            "//:dynamic.bxl:dynamic_test_execution_resolution",
        ),
        stderr_regex="Anon target or dynamic action accesses bxl.Actions.exec_deps",
    )


@yak_test()
async def test_bxl_dynamic_incompatible_targets(yak: Yak) -> None:
    result = await yak.bxl(
        "//:dynamic.bxl:dynamic_test_incompatible_targets",
    )

    assert "Skipped 1 incompatible targets" in result.stderr
