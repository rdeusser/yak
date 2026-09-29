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


@yak_test()
async def test_package_file_alt_name(yak: Yak) -> None:
    output = await yak.build("//:")
    assert "AAA from YAK_TREE" in output.stderr
    assert "AAA from PACKAGE" not in output.stderr

    os.unlink(yak.cwd / "YAK_TREE")

    output = await yak.build("//:")
    assert "AAA from YAK_TREE" not in output.stderr
    assert "AAA from PACKAGE" in output.stderr
