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


@yak_test()
async def test_compatible_with(yak: Yak) -> None:
    for good in ["root//:pass", "root//:pass2"]:
        out = await yak.cquery(good)
        assert re.match(
            "{} \\(.*\\)\n".format(good),
            out.stdout,
        )

    for bad in ["root//:fail", "root//:fail2"]:
        out = await yak.cquery(bad)
        assert out.stdout == ""
