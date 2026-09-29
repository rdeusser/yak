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
async def test_prelude_starlark_unit_tests(yak: Yak) -> None:
    # The build file calls the test functions in the `*_tests.bzl` files, which
    # check prelude utilities against the bundled prelude with `asserts`. A failed
    # assertion fails the evaluation of the build file.
    await yak.targets("root//:")
