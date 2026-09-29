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
async def test_emit_console_preferences(yak: Yak) -> None:
    await yak.build("-c", "ui.thread_line_limit=30")

    max_lines = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "ConsolePreferences",
        "max_lines",
    )

    assert max_lines
    assert max_lines[0] == 30
