# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e.configurations.cfg_constructor.modifiers_util import get_cfg
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_cfg_modifiers_attr(yak: Yak) -> None:
    result = await yak.targets(
        "root//:test",
        "--output-attribute=modifiers",
    )

    targets = json.loads(result.stdout)
    assert len(targets) == 1
    target = targets[0]
    target_modifiers = target["modifiers"]
    assert target_modifiers == ["root//:A_1"]


@yak_test()
async def test_cfg_modifiers_attr_ctargets(yak: Yak) -> None:
    result = await get_cfg(
        yak,
        "root//:test2",
    )
    assert ":A_1" in result


@yak_test()
async def test_metadata_modifiers_is_hard_error(yak: Yak) -> None:
    result = await expect_failure(yak.ctargets("root//:test_metadata_modifiers"))
    assert (
        'sets `metadata["yak.cfg_modifiers"]` which is no longer supported'
        in result.stderr
    )
