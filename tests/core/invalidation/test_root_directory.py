# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import json_get


@yak_test()
async def test_no_dice_invalidation_on_root_directory_changes(yak: Yak) -> None:
    await yak.build("root//dir:")

    # Add a file to the root directory
    (yak.cwd / "file.txt").write_text("hello world")

    await yak.build("root//dir:")

    log = (await yak.log("show")).stdout.splitlines()

    for line in log:
        e = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "Load",
        )
        assert e is None, "Should not have loaded anything"
