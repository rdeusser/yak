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
async def test_build_skip_missing(yak: Yak) -> None:
    result = await yak.build(
        "//:existing",
        "//:missing",
        "--skip-missing-targets",
    )

    out = result.get_build_report().output_for_target("//:existing").read_text()
    assert "abcd" == out.strip()
    assert "Skipped 1 missing targets:" in result.stderr


@yak_test()
async def test_build_skip_missing_fails_on_missing_package(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:existing",
            "//bad-package:existing",
            "--skip-missing-targets",
        ),
        stderr_regex="root//bad-package",
    )
