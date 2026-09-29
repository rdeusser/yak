# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_daemon_buster(yak: Yak) -> None:
    async def pid() -> int:
        return json.loads((await yak.status()).stdout)["process_info"]["pid"]

    await yak.build(":")
    pid0 = await pid()

    await yak.build(":")
    pid1 = await pid()
    assert pid1 == pid0

    with open(yak.cwd / ".yakconfig", "a") as f:
        f.write("[yak]\n")
        f.write("daemon_buster = 1\n")

    await yak.build(":")
    pid2 = await pid()
    assert pid2 != pid1

    await yak.build(":")
    pid3 = await pid()
    assert pid3 == pid2

    with open(yak.cwd / ".yakconfig", "a") as f:
        f.write("[yak]\n")
        f.write("daemon_buster = 2\n")

    await yak.build(":")
    pid4 = await pid()
    assert pid4 != pid3

    with open(yak.cwd / ".yakconfig", "r") as f:
        config = f.read()

    with open(yak.cwd / ".yakconfig", "w") as f:
        f.write(
            "\n".join(
                line for line in config.splitlines() if "daemon_buster" not in line
            )
        )

    await yak.build(":")
    pid5 = await pid()
    assert pid5 != pid4
