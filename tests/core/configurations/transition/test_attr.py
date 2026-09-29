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
async def test_configuration_transition_attr(yak: Yak) -> None:
    result = await yak.cquery("deps(root//:the-test)")
    result.check_returncode()
    # Default configuration is iphoneos and it should be transitioned to watchos
    assert ":watchos_resource" in result.stdout
    assert ":default_resource" not in result.stdout
