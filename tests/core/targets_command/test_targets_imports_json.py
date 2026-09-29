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
async def test_imports_json(yak: Yak) -> None:
    """Test that targets --streaming --imports handles JSON file imports."""
    result = await yak.targets("//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)

    found_targets = False
    found_bzl = False
    found_json = False

    for x in xs:
        if "yak.imports" not in x:
            continue
        file = x["yak.file"]
        imports = x["yak.imports"]

        if file == "root//YAK.fixture":
            assert "root//uses_json.bzl" in imports
            found_targets = True
        elif file == "root//uses_json.bzl":
            assert "root//data.json" in imports
            found_bzl = True
        elif file == "root//data.json":
            assert imports == []
            found_json = True

    assert found_targets, "YAK.fixture imports should be reported"
    assert found_bzl, "uses_json.bzl imports (including data.json) should be reported"
    assert found_json, "data.json should appear as an import with empty sub-imports"
