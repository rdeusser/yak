# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_http2_enabled(yak: Yak) -> None:
    # Get a daemon to start
    await yak.build()
    result = await yak.status()
    status = json.loads(result.stdout)
    assert status["http2"] is True, "http2 is enabled by default"

    # Insert necessary yakconfig to pick up http2 configuration.
    with open(f"{yak.cwd}/.yakconfig", "a") as yakconfig:
        yakconfig.writelines(["[http]\n", "http2 = false\n"])

    # Get a daemon to start
    await yak.build()
    result = await yak.status()
    status = json.loads(result.stdout)
    assert status["http2"] is False, "http2 was disabled by yakconfig"
