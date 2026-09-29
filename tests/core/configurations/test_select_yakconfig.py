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


# Test select works with yakconfig.
@yak_test()
async def test_select_yakconfig(yak: Yak) -> None:
    out = await yak.cquery(
        "root//:the-test",
        "--output-attribute=labels",
    )
    q = json.loads(out.stdout)
    assert len(q) == 1
    assert list(q.values())[0]["labels"] == ["NO"]

    out = await yak.cquery(
        "root//:the-test",
        "--output-attribute=labels",
        "-c",
        "aaa.bbb=ccc",
    )
    q = json.loads(out.stdout)
    assert len(q) == 1
    assert list(q.values())[0]["labels"] == ["YES"]


@yak_test()
async def test_select_root_yakconfig(yak: Yak) -> None:
    out = await yak.cquery(
        "subcell//:the-root-yakconfig-test",
        "--output-attribute=labels",
    )
    q = json.loads(out.stdout)
    assert len(q) == 1
    assert set(list(q.values())[0]["labels"]) == {"ROOT_YES", "TARGET_YES"}
