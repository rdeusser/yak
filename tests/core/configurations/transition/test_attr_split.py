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
async def test_configuration_transition_attr_split_cquery(yak: Yak) -> None:
    result = await yak.cquery("deps(root//:bb)")
    result.check_returncode()
    # Check both transitioned deps are present.
    assert "root//:code (arm64#" in result.stdout
    assert "root//:code (arm32#" in result.stdout


@yak_test()
async def test_configuration_transition_attr_split_build(yak: Yak) -> None:
    result = await yak.build("root//:bb")
    result.check_returncode()
    # Rule implementations do the assertions.
