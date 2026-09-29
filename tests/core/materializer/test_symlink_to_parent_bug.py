# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test(skip_for_os=["windows"])
async def test_symlink_to_parent_bug(yak: Yak) -> None:
    result = await yak.build("//:whistle", "--prefer-local", "--no-remote-cache")
    out = result.get_build_report().output_for_target("//:whistle")
    assert str(out).endswith("/whistle")
    # Check the link was actually materialized.
    assert os.path.islink(out)


@yak_test(skip_for_os=["windows"])
async def test_symlink_to_self(yak: Yak) -> None:
    result = await yak.build("//:flute", "--prefer-local", "--no-remote-cache")
    out = result.get_build_report().output_for_target("//:flute")
    assert str(out).endswith("/flute")
    # Check the link was actually materialized.
    assert os.path.islink(out)
