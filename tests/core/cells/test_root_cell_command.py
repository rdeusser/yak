# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_root_cell_with_ignored_yakconfig(yak: Yak) -> None:
    r = await yak.root("--kind=cell", rel_cwd=Path("abc"))
    assert r.stdout.strip() == str(yak.cwd)
