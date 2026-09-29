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
async def test_audit_deferred_materializer_list(yak: Yak) -> None:
    res = await yak.audit("deferred-materializer", "list")
    assert res.stdout.strip() == ""

    await yak.build("//:simple")

    res = await yak.audit("deferred-materializer", "list")
    assert "__simple__" in res.stdout.strip()
