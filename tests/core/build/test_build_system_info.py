# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


@yak_test()
async def test_build_system_info(yak: Yak) -> None:
    await yak.build(
        "//:test",
    )

    system_info = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "SystemInfo",
    )
    assert len(system_info) == 1
    assert system_info[0]["system_total_memory_bytes"] > 0
    assert system_info[0]["total_disk_space_bytes"] > 0
