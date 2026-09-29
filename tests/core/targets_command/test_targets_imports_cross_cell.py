# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_imports_cross_cell(yak: Yak) -> None:
    # `nested//` has no `PACKAGE` files of its own, but the root cell's
    # `PACKAGE` file applies to it, so the imports walk must report that
    # file and its loads, matching `PACKAGE` evaluation.
    result = await yak.targets("nested//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)
    files = {x["yak.file"]: x for x in xs if "yak.imports" in x}

    assert "root//PACKAGE" in files
    package_record = files["root//PACKAGE"]
    assert package_record["yak.imports"] == [
        "prelude//prelude.bzl",
        "root//a.bzl",
    ]
    assert "yak.package" not in package_record

    # The PACKAGE file's own imports are chased transitively.
    assert "root//a.bzl" in files
