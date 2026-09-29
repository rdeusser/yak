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
async def test_imports(yak: Yak) -> None:
    result = await yak.targets("//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)
    found = 0
    for x in xs:
        if "yak.imports" in x:
            if x["yak.file"] == "root//YAK.fixture":
                assert x["yak.package"] == "root//"
                assert x["yak.imports"] == ["prelude//prelude.bzl", "root//a.bzl"]
                found += 1
            elif x["yak.file"] == "root//a.bzl":
                assert x["yak.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "yak.package" not in x
                found += 1
            elif x["yak.file"] == "root//PACKAGE":
                assert x["yak.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "yak.package" not in x
                found += 1
    assert found == 3
