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
async def test_invoke_cfg_constructors(yak: Yak) -> None:
    result = await yak.cquery("root//:test")
    assert "root//:test (post_constraint_analysis_test_label" in result.stdout


@yak_test()
async def test_invoke_cfg_constructors_without_aliases(yak: Yak) -> None:
    # This test ensures that for backwards compatibility, we can call
    # `set_cfg_constructor` without explicitly passing in aliases parameter.
    result = await yak.cquery("root//:test", "-c", "testing.no_aliases=true")
    assert "root//:test (post_constraint_analysis_test_label" in result.stdout


@yak_test()
async def test_invoke_cfg_constructors_unbound_platform(yak: Yak) -> None:
    result = await yak.cquery("root//:test_unbound")
    assert "root//:test_unbound (post_constraint_analysis_test_label" in result.stdout
