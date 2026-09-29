# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import re
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakResult
from e2e_util.yak_workspace import yak_test


def _includes(output: YakResult) -> list[str]:
    return sorted(
        [
            re.sub(".*[/\\\\]", "", line)
            for line in output.stdout.splitlines()
            if line.endswith(".bzl") or line.endswith(".json")
        ]
    )


@yak_test()
async def test_audit_includes(yak: Yak, tmp_path: Path) -> None:
    expected_includes = ["example.json", "incl.bzl", "prelude.bzl"]
    # Using project relative path.
    output = await yak.audit("includes", "YAK.fixture")
    assert _includes(output) == expected_includes

    # Using project relative path when in a subdirectory.
    await yak.audit("includes", "YAK.fixture", rel_cwd=Path("dir"))
    assert _includes(output) == expected_includes

    # Using absolute path.
    output = await yak.audit("includes", f"{yak.cwd}/YAK.fixture")
    assert _includes(output) == expected_includes

    if os.name != "nt":
        # Create symlink to the project root in a temporary directory.
        (tmp_path / "symlink").symlink_to(yak.cwd)

        output = await yak.audit("includes", f"{tmp_path}/symlink/YAK.fixture")
        assert _includes(output) == expected_includes
