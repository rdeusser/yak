# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_default_target_platform_is_subtarget(yak: Yak) -> None:
    # FIXME(JakobDegen): Bug. The target specifies a subtarget that does have an appropriate
    # provider.
    await expect_failure(
        yak.cquery(":stub"),
        stderr_regex="Expected `root//:alias_platform` to be a `platform\\(\\)` target",
    )


@yak_test()
async def test_subtarget_in_select_key(yak: Yak) -> None:
    res = await yak.uquery(
        "root//:with_constraint_key_dep", "-a", "yak.configuration_deps"
    )
    res = json.loads(res.stdout)
    # FIXME(JakobDegen): Bug. `yak.deps`-like attributes do not include subtargets
    assert list(res.values())[0]["yak.configuration_deps"] == ["root//:cat_alias[sub]"]
