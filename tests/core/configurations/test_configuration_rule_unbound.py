# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_configuration_rule_unbound(yak: Yak) -> None:
    result = await yak.cquery(
        # platform argument is ignored
        "--target-platforms=root//:p",
        "root//:the-test",
    )
    # Note configuration is unbound here.
    assert "root//:the-test (<unbound>)\n" == result.stdout
