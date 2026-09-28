# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.buck import Buck
from e2e_util.asserts import expect_failure
from e2e_util.buck_workspace import buck_test


@buck_test()
async def test_invoke_cfg_constructors_bad_constraints(buck: Buck) -> None:
    result = await expect_failure(buck.cquery("root//:test"))
    assert "root//:not_a_constraint is not a configuration rule." in result.stderr
