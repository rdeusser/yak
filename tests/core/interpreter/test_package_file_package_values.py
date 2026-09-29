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


@yak_test()
async def test_package_file_package_values(yak: Yak) -> None:
    # Build file does all the assertions.
    output = await yak.build("//:")
    assert "TEST PASSED" in output.stderr


@yak_test()
async def test_audit_package_values(yak: Yak) -> None:
    stdout = (await yak.audit("package-values", "//")).stdout
    golden(
        output=stdout,
        rel_path="audit-package-values.golden.json",
    )


@yak_test()
async def test_audit_package_values_select(yak: Yak) -> None:
    stdout = (await yak.audit("package-values", "//")).stdout
    result = json.loads(stdout)
    pkg = result["root//"]
    # Verify select value has expected JSON structure
    assert pkg["sel.ector"] == {
        "__type": "selector",
        "entries": {"//config:a": "val_a", "DEFAULT": "default_val"},
    }
    # Verify concat (select + select) has expected JSON structure
    assert pkg["sel.concat"]["__type"] == "concat"
    assert len(pkg["sel.concat"]["items"]) == 2
    # Verify visibility fields exist alongside package values
    assert "visibility" in pkg
    assert "within_view" in pkg
    assert "visibility_cap" in pkg
    assert "within_view_cap" in pkg


@yak_test()
async def test_audit_package_values_visibility_cap_intersection(
    yak: Yak,
) -> None:
    stdout = (await yak.audit("package-values", "//capped/child")).stdout
    result = json.loads(stdout)
    pkg = result["root//capped/child"]
    cap = pkg["visibility_cap"]
    assert isinstance(cap, dict), f"Expected dict for intersection, got {type(cap)}"
    assert "intersection" in cap
    assert len(cap["intersection"]) == 2


@yak_test()
async def test_targets_package_values(yak: Yak) -> None:
    stdout = (await yak.targets("--package-values", "//...")).stdout
    golden(
        output=stdout,
        rel_path="targets-package-values.golden.json",
    )


@yak_test()
async def test_targets_package_values_regex(yak: Yak) -> None:
    # Empty string as regex matches all keys.
    out = (await yak.targets("--package-values-regex", "", "//...")).stdout
    json_result = json.loads(out)[0]
    pv = json_result["yak.package_values"]
    assert pv["aaa.bbb"] == "ccc"
    assert pv["xxx.yyy"] == "zzz"
    assert pv["sel.ector"]["__type"] == "selector"
    assert pv["sel.concat"]["__type"] == "concat"

    out = (await yak.targets("--package-values-regex", "aaa.bbb", "//...")).stdout
    json_result = json.loads(out)[0]
    expected = {"aaa.bbb": "ccc"}
    assert json_result["yak.package_values"] == expected

    out = (await yak.targets("--package-values-regex", "xxx", "//...")).stdout
    json_result = json.loads(out)[0]
    expected = {"xxx.yyy": "zzz"}
    assert json_result["yak.package_values"] == expected

    out = (
        await yak.targets(
            "--package-values-regex",
            "aaa.bbb",
            "--package-values-regex",
            "xxx.yyy",
            "//...",
        )
    ).stdout
    json_result = json.loads(out)[0]
    expected = {"aaa.bbb": "ccc", "xxx.yyy": "zzz"}
    assert json_result["yak.package_values"] == expected

    # Regex matching select keys.
    out = (await yak.targets("--package-values-regex", "sel", "//...")).stdout
    json_result = json.loads(out)[0]
    pv = json_result["yak.package_values"]
    assert len(pv) == 2
    assert pv["sel.ector"]["__type"] == "selector"
    assert pv["sel.concat"]["__type"] == "concat"

    out = (await yak.targets("--package-values-regex", "non_existent", "//...")).stdout
    json_result = json.loads(out)[0]
    expected = {}
    assert json_result["yak.package_values"] == expected

    args = ["allow", "only", "one", "arg", "per", "flag", "occurrence"]
    await expect_failure(
        yak.targets("--package-values-regex", *args, "//..."),
        stderr_regex="Error parsing root//arg",
    )


@yak_test()
async def test_targets_streaming_package_values(yak: Yak) -> None:
    stdout = (await yak.targets("--streaming", "--package-values", "//...")).stdout
    golden(
        output=stdout,
        rel_path="targets-streaming-package-values.golden.json",
    )
