# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test(data_dir="simple")
async def test_query_owner(yak: Yak) -> None:
    result = await yak.cquery(
        "--target-universe=root//bin:the_binary", """owner(bin/YAK.fixture)"""
    )
    assert (
        _replace_hash(result.stdout)
        == "root//bin:the_binary (root//platforms:platform1#<HASH>)\n"
    )
