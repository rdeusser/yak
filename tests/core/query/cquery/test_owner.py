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


@yak_test(data_dir="deprecated_correct")
async def test_owner_without_universe_correct(yak: Yak) -> None:
    # TODO(nga): there should be a warning.
    result = await yak.cquery(
        "owner(bin.sh)",
    )
    assert "" == result.stdout
    assert (
        "Query has no target literals and `--target-universe` is not specified"
        in result.stderr
    )


@yak_test(data_dir="deprecated_correct")
async def test_owner_with_auto_universe_correct(yak: Yak) -> None:
    result = await yak.cquery(
        "deps(//:test) intersect owner(bin.sh)",
    )
    lines = result.stdout.splitlines()
    # Drop configuration.
    targets = [t.split()[0] for t in lines]
    assert ["root//:bin"] == targets
