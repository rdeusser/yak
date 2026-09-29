# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test()
async def test_ctargets_basic(yak: Yak) -> None:
    result = await yak.ctargets(
        "root//:gum",
        "--target-platforms=root//:p",
    )
    [line] = result.stdout.splitlines()
    line = _replace_hash(line)
    assert line == "root//:gum (root//:p#<HASH>)"


@yak_test()
async def test_ctargets_json(yak: Yak) -> None:
    result = await yak.ctargets(
        "root//:chocolate",
        "--json",
    )

    [output] = json.loads(result.stdout)

    output["yak.type"]
    output["yak.deps"]
    output["yak.inputs"]
    output["yak.package"]
    output["name"]
    assert output["default_target_platform"] == "root//:p"
    output["visibility"]
    output["within_view"]


@yak_test()
async def test_ctargets_multi_json(yak: Yak) -> None:
    result = await yak.ctargets(
        "root//:",
        "--json",
    )

    outputs = json.loads(result.stdout)

    assert len(outputs) == 3

    for output in outputs:
        output["yak.type"]
        output["yak.deps"]
        output["yak.inputs"]
        output["yak.package"]

        name = output["name"]
        if name == "chocolate":
            assert output["default_target_platform"] == "root//:p"

        output["visibility"]
        output["within_view"]


@yak_test()
async def test_ctargets_output_attribute(yak: Yak) -> None:
    result = await yak.ctargets(
        "root//:chocolate", "--output-attribute=default_*", "--output-attribute=name"
    )

    [output] = json.loads(result.stdout)

    assert {
        "name": "chocolate",
        "default_target_platform": "root//:p",
    } == output
