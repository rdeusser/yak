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
async def test_local_test_execution_not_cached(yak: Yak) -> None:
    seed = random_string()
    args = [
        "-c",
        "test.local_enabled=true",
        "-c",
        "test.remote_enabled=false",
        "-c",
        f"test.seed={seed}",
        "//:cacheable_test",
    ]

    await yak.test(*args)

    await yak.test(*args)
    second_what_ran = await read_what_ran(yak)
    second_test_runs = [
        entry for entry in second_what_ran if entry["reason"] == "test.run"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one test.run entry, got {len(second_test_runs)}"
    )
    assert second_test_runs[0]["reproducer"]["executor"] == "Local", (
        "Expected test to run locally, not be cached!"
    )


@pytest.mark.remote_execution
@yak_test()
async def test_remote_test_execution_not_cached_with_no_remote_cache(
    yak: Yak,
) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "--no-remote-cache",
        "//:cacheable_test",
    ]

    await yak.test(*args)

    await yak.test(*args)
    second_what_ran = await read_what_ran(yak)
    second_test_runs = [
        entry for entry in second_what_ran if entry["reason"] == "test.run"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one test.run entry, got {len(second_test_runs)}"
    )
    assert second_test_runs[0]["reproducer"]["executor"] == "Re", (
        "Expected test to run remotely, not be cached!"
    )
