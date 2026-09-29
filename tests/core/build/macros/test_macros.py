# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import platform

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_run_with_source_macros(yak: Yak) -> None:
    sep = "\\" if platform.system() == "Windows" else "/"
    result = await yak.run("//source:echo_file")
    assert result.stdout.endswith(f"source{sep}foo.txt\n")

    result = await yak.run("//source:echo_dir")
    assert result.stdout.endswith(f"source{sep}bar\n")

    result = await yak.run("//source:cat_file")
    assert result.stdout == "foo file\n"

    result = await yak.run("//source:cat_dir")
    assert result.stdout == "bar file\n"


@yak_test()
async def test_no_dep_in_source(yak: Yak) -> None:
    await expect_failure(
        yak.build("//dep_as_source:uses_dep"),
        stderr_regex="Source file `:trivial` does not exist",
    )
