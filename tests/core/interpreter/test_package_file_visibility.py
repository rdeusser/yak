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
async def test_no_package_call_does_not_reset_visibility(yak: Yak) -> None:
    # Test that PACKAGE file without package() call does not reset visibility inherited from parent PACKAGE file.

    await yak.build("root//b:top")
