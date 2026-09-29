# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import json

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import random_string


@yak_test()
async def test_incremental_file_materialized(yak: Yak) -> None:
    result = await yak.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await yak.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@yak_test()
async def test_incremental_dir_materialized(yak: Yak) -> None:
    result = await yak.run("root//:mate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await yak.run("root//:mate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@yak_test()
async def test_incremental_file_not_materialized(yak: Yak) -> None:
    result = await yak.run("root//:flute", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await yak.run("root//:flute", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@yak_test()
async def test_incremental_dir_not_materialized(yak: Yak) -> None:
    result = await yak.run("root//:suite", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await yak.run("root//:suite", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@pytest.mark.remote_execution
@yak_test()
async def test_remote_cache_is_used(yak: Yak) -> None:
    seed = random_string()
    result = await yak.run("root//:plate", "-c", f"test.seed={seed}")
    assert result.stdout == "0"
    result = await yak.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"

    # For the next build with already used seed we expect the action to be taken from the cache
    result = await yak.run("root//:plate", "-c", f"test.seed={seed}")
    assert result.stdout == "0"

    out = await yak.log("what-ran", "--format", "json")
    out = [line.strip() for line in out.stdout.splitlines()]
    out = [json.loads(line) for line in out if line]
    assert len(out) == 1, "out should have 1 line: `{}`".format(out)
    repro = out[0]
    assert repro["reproducer"]["executor"] == "Cache"
