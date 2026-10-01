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
from e2e_util.helper.utils import random_string, read_what_ran


@pytest.mark.remote_execution
@yak_test()
async def test_stable_action_digest_with_deterministic_paths(yak: Yak) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:test",
    ]

    await yak.test(*args)
    first_what_ran = await read_what_ran(yak)
    first_digests = [
        entry["reproducer"]["details"]["digest"]
        for entry in first_what_ran
        if entry["reason"] == "test.run"
    ]
    assert len(first_digests) == 1, "Expected one test.run entry"

    await yak.test(*args)
    second_what_ran = await read_what_ran(yak)
    second_digests = [
        entry["reproducer"]["details"]["digest"]
        for entry in second_what_ran
        if entry["reason"] == "test.run"
    ]
    assert len(second_digests) == 1, "Expected one test.run entry"

    assert first_digests[0] == second_digests[0], (
        f"Test action digests differ between runs: {first_digests[0]} vs {second_digests[0]}"
    )


@pytest.mark.remote_execution
@yak_test()
async def test_remote_test_execution_cached(yak: Yak) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:cacheable_test",
    ]

    await yak.test(*args)

    await yak.test(*args)
    second_what_ran = await read_what_ran(yak, "--emit-cache-queries")
    second_test_runs = [
        entry
        for entry in second_what_ran
        if entry["reason"] == "test.run"
        and entry.get("reproducer", {}).get("executor") == "Cache"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one cached test.run entry, got {len(second_test_runs)}"
    )


@yak_test()
async def test_local_test_execution_cached(yak: Yak) -> None:
    seed = random_string()
    config = [
        "-c",
        "test.local_enabled=true",
        "-c",
        "test.remote_enabled=false",
        "-c",
        f"test.seed={seed}",
    ]

    async def second_run_executor(target: str) -> str:
        await yak.test(*config, target)
        await yak.test(*config, target)
        test_runs = [
            entry for entry in await read_what_ran(yak) if entry["reason"] == "test.run"
        ]
        assert len(test_runs) == 1, (
            f"Expected exactly one test.run entry, got {len(test_runs)}"
        )
        return test_runs[0]["reproducer"]["executor"]

    # A pass of a test that supports caching counts for the next run, and a
    # pass of any other test does not.
    assert await second_run_executor("//:cacheable_test") == "LocalCache"
    assert await second_run_executor("//:test") == "Local"
