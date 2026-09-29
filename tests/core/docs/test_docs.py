# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


"""
Tests to ensure that the `yak docs` command works as expected
"""


@yak_test()
async def test_docs_returns(yak: Yak) -> None:
    result = await yak.docs("starlark")
    result.check_returncode()
    decoded = json.loads(result.stdout)
    assert decoded == []


@yak_test()
async def test_prelude_docs(yak: Yak) -> None:
    result = await yak.docs("starlark", "prelude//:prelude.bzl")
    result.check_returncode()
    decoded = json.loads(result.stdout)
    golden(
        output=json.dumps(decoded, indent=2),
        rel_path="prelude_docs.golden.json",
    )


@yak_test()
async def test_docs_fail_with_invalid_patterns(yak: Yak) -> None:
    await expect_failure(
        yak.docs("starlark", "not_an_import_path"),
        stderr_regex="Import path must have suffix .*: `root//not_an_import_path`",
    )
    await expect_failure(
        yak.docs("starlark", "//cell"),
        stderr_regex="Import path must have suffix .*: `root//cell`",
    )
