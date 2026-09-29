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
async def test_dice_is_not_invalidated_on_changes_in_ignored_directories(
    yak: Yak,
) -> None:
    await yak.targets("root//...")
    (yak.cwd / "dir" / "fignore").write_text("xyz")
    await yak.targets("root//...")
    dice_equal = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "DiceEqualityCheck",
        "is_equal",
    )
    assert dice_equal == [True]
