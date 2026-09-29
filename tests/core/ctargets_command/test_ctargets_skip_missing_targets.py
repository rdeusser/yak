# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test()
async def test_ctargets_skip_missing_targets(yak: Yak) -> None:
    await expect_failure(
        yak.ctargets(
            "root//:existing",
            "root//:nonexistent",
            "--target-platforms=root//:p",
        ),
        stderr_regex="Unknown target `nonexistent` from package",
    )

    result = await yak.ctargets(
        "root//:existing",
        "root//:nonexistent",
        "--target-platforms=root//:p",
        "--skip-missing-targets",
    )
    [line] = result.stdout.splitlines()
    line = _replace_hash(line)
    assert line == "root//:existing (root//:p#<HASH>)"

    assert "Skipped 1 missing targets:" in result.stderr
    assert "root//:nonexistent" in result.stderr
