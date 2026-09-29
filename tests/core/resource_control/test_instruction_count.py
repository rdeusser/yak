# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
from typing import Any, Dict, List

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events, random_string

pytestmark = pytest.mark.needs_binary("THREE_BILLION_INSTRUCTIONS_BIN")


def helper_bin_flags() -> List[str]:
    return [
        "-c",
        f"three_billion_instructions.path={os.environ['THREE_BILLION_INSTRUCTIONS_BIN']}",
    ]


@yak_test(skip_for_os=["windows", "darwin"], disable_daemon_cgroup=False)
async def test_instruction_count_disabled(yak: Yak) -> None:
    await yak.build(
        "root//:three_billion_instructions",
        "-c",
        "yak.miniperf2=false",
        "--no-remote-cache",
        "--local-only",
        "-c",
        f"test.cache_buster={random_string()}",
        *helper_bin_flags(),
    )

    events = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "commands",
    )
    for commands in events:
        for c in commands:
            assert c["details"]["metadata"].get("execution_stats") is None


async def get_matching_details(yak: Yak) -> Dict[str, Any]:
    events = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
    )
    for action in events:
        if action["name"]["category"] == "three_billion_instructions":
            return action["commands"][-1]["details"]

    raise AssertionError("did not find the expected target")


@yak_test(skip_for_os=["windows", "darwin"], disable_daemon_cgroup=False)
async def test_instruction_count_enabled(yak: Yak) -> None:
    await yak.build(
        "root//:three_billion_instructions",
        "-c",
        "yak.miniperf2=true",
        "--no-remote-cache",
        "--local-only",
        "-c",
        f"test.cache_buster={random_string()}",
        *helper_bin_flags(),
    )

    details = await get_matching_details(yak)
    assert "OmittedLocalCommand" in details["command_kind"]["command"]

    # Check that we are within 20%
    instruction_count = details["metadata"]["execution_stats"]["cpu_instructions_user"]
    # FIXME(JakobDegen): Are we really expecting downward variation? Why? Leave a comment
    assert instruction_count > 2700000000
    assert instruction_count < 3300000000


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows", "darwin"], disable_daemon_cgroup=False)
async def test_instruction_count_remote(yak: Yak) -> None:
    await yak.build(
        "root//:three_billion_instructions",
        "--no-remote-cache",
        "--write-to-cache-anyway",
        "--prefer-remote",
        *helper_bin_flags(),
    )

    details = await get_matching_details(yak)
    assert not details["command_kind"]["command"]["RemoteCommand"]["cache_hit"]

    # Check that we are within 10%
    instruction_count = details["metadata"]["execution_stats"]["cpu_instructions_user"]
    assert instruction_count > 2850000000
    assert instruction_count < 3150000000

    # Check we also get it on a cache hit.

    await yak.kill()
    await yak.build(
        "root//:three_billion_instructions",
        "--prefer-remote",
        *helper_bin_flags(),
    )

    details = await get_matching_details(yak)
    assert details["command_kind"]["command"]["RemoteCommand"]["cache_hit"]

    # Check that we are within 10%
    instruction_count = details["metadata"]["execution_stats"]["cpu_instructions_user"]
    assert instruction_count > 2850000000
    assert instruction_count < 3150000000
