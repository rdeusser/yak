# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_within_view(yak: Yak) -> None:
    res = await yak.targets("//a/...", "--json-lines")
    assert res.get_target_list() == ["prelude//a:a"]


@yak_test()
async def test_within_view_outside_view(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//b/..."),
        stderr_regex="Target's `within_view` attribute does not allow dependency `prelude//a:a`",
    )


@yak_test()
async def test_within_view_default_outofview(yak: Yak) -> None:
    res = await yak.targets("//default/...", "--json-lines")
    assert res.get_target_list() == ["prelude//default:a"]


@yak_test()
async def test_within_view_default_outofview_withnone(yak: Yak) -> None:
    res = await yak.targets("//default_withnone/none/...", "--json-lines")
    assert res.get_target_list() == [
        "prelude//default_withnone/none:target",
    ]


@yak_test()
async def test_within_view_default_outofview_withnoneselect(yak: Yak) -> None:
    res = await yak.targets("//default_withnone/select/...", "--json-lines")
    assert res.get_target_list() == [
        "prelude//default_withnone/select:target",
    ]


@yak_test()
async def test_within_view_default_outofview_withdefault(yak: Yak) -> None:
    res = await yak.targets("//default_withvalue/value/...", "--json-lines")
    assert res.get_target_list() == [
        "prelude//default_withvalue/value:target",
    ]


@yak_test()
async def test_within_view_default_outofview_withdefaultselect(yak: Yak) -> None:
    res = await yak.targets("//default_withvalue/select/...", "--json-lines")
    assert res.get_target_list() == [
        "prelude//default_withvalue/select:target",
    ]
