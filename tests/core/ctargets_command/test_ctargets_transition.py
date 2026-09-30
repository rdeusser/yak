# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import json
import re

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@pytest.mark.parametrize("output_format", ["text", "json", "json-report"])
@pytest.mark.parametrize("hash_mode", ["none", "non_recursive", "recursive"])
@yak_test()
async def test_ctargets_transition(
    yak: Yak, output_format: str, hash_mode: str
) -> None:
    args = []
    if output_format != "text":
        args.append(f"--{output_format}")
    if hash_mode != "none":
        args.append("--show-target-hash")
    if hash_mode == "recursive":
        args.append("--target-hash-recursive")
    result = await yak.ctargets(
        "root//:candy",
        "--target-platforms=root//:p",
        *args,
    )
    output = _replace_hash(result.stdout)
    if output_format == "text":
        [line] = output.splitlines()
        if hash_mode != "none":
            line, target_hash = line.rsplit(" ", 1)
            assert re.fullmatch(r"[0-9a-f]{32}", target_hash)
        assert line == "root//:candy (<clay>#<HASH>)"
        return

    targets = json.loads(output)
    if output_format == "json-report":
        assert targets["incompatible_targets"] == []
        targets = targets["compatible_targets"]
    [target] = targets
    assert target["yak.target"] == "root//:candy"
    assert target["yak.type"] == "root//defs.bzl:clay_library"
    assert target["value"] == "before"
    if hash_mode == "none":
        assert "yak.target_hash" not in target
    else:
        assert re.fullmatch(r"[0-9a-f]{32}", target["yak.target_hash"])


@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_ctargets_transition_hash_tracks_actual_attributes(
    yak: Yak, hash_function: str, recursive: bool
) -> None:
    args = [
        "root//:candy",
        "--target-platforms=root//:p",
        "--json",
        "--show-target-hash",
        f"--target-hash-function={hash_function}",
    ]
    if recursive:
        args.append("--target-hash-recursive")
    [before] = json.loads((await yak.ctargets(*args)).stdout)
    [after] = json.loads((await yak.ctargets(*args, "-c", "test.value=after")).stdout)
    assert before["value"] == "before"
    assert after["value"] == "after"
    assert before["yak.target_hash"] != after["yak.target_hash"]
