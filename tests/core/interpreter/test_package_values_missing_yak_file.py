# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


@yak_test()
async def test_package_values_missing_yak_file(yak: Yak) -> None:
    stdout = (await yak.audit("package-values", "//")).stdout
    golden(
        output=stdout,
        rel_path="audit-package-values-missing-yak-file.golden.json",
    )
