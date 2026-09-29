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


@yak_test()
async def test_deps_in_cquery_not_uquery(yak: Yak) -> None:
    # Check that plugin deps appear as deps in uquery but not in cquery
    result = await yak.uquery("deps(//tests:reg_a)")
    assert "//tests:reg_a_REAL" in result.stdout
    result = await yak.cquery("deps(//tests:reg_a)")
    assert "//tests:reg_a_REAL" not in result.stdout
    # And make sure that the attribute itself is serialized correctly in cquery and uquery
    result = await yak.uquery("-a", "actual", "//tests:reg_a")
    assert json.loads(result.stdout) == {
        "root//tests:reg_a": {"actual": "root//tests:reg_a_REAL"}
    }
    result = await yak.cquery("-a", "actual", "//tests:reg_a")
    assert json.loads(result.stdout) == {
        "root//tests:reg_a (<unspecified>)": {"actual": "root//tests:reg_a_REAL"}
    }


@yak_test()
async def test_cquery(yak: Yak) -> None:
    ###### Check that everything is correctly configured as reported by cquery
    result = await yak.cquery(
        "--json",
        "-a",
        "yak.deps",
        "-a",
        "yak.execution_platform",
        "-a",
        "yak.plugins",
        "deps(//tests:b)",
    )
    result = json.loads(result.stdout)

    b = next(v for k, v in result.items() if k.startswith("root//tests:b"))
    l = next(  # noqa: E741 `l` as a variable name is fine
        v for k, v in result.items() if k.startswith("root//tests:l")
    )

    assert set(b["yak.plugins"]["RustProcMacro"]) == {
        "root//tests:reg_a_REAL",
        "root//tests:reg_b_REAL",
        "root//tests:doc_a_REAL",
    }
    assert set(l["yak.plugins"]["RustProcMacro"]) == {
        "root//tests:reg_a_REAL",
        "root//tests:doc_b_REAL",
    }

    assert b["yak.execution_platform"].startswith("root//config:platform_linux")
    assert any(
        dep.startswith("root//tests:reg_a_REAL (root//config:platform_linux")
        for dep in b["yak.deps"]
    )
    assert l["yak.execution_platform"].startswith("root//config:platform_windows")
    assert any(
        dep.startswith("root//tests:reg_a_REAL (root//config:platform_windows")
        for dep in l["yak.deps"]
    )

    assert any(
        k.startswith("root//tests:reg_a_REAL (root//config:platform_linux")
        for k in result.keys()
    )
    assert any(
        k.startswith("root//tests:reg_a_REAL (root//config:platform_windows")
        for k in result.keys()
    )


@yak_test()
async def test_analysis(yak: Yak) -> None:
    # Check that we can properly identify all the different plugin deps in analysis
    result = await yak.build("root//tests:b", "root//tests:l")

    b = json.loads(
        result.get_build_report().output_for_target("root//tests:b").read_text()
    )
    assert b == {
        "indirect": ["Reg A (linux)"],
        "direct": ["Reg B (linux)"],
        "indirect_doc": ["Doc A (linux)"],
        "direct_doc": [],
    }

    l = json.loads(  # noqa: E741 `l` as a variable name is fine
        result.get_build_report().output_for_target("root//tests:l").read_text()
    )
    assert l == {  # noqa: E741 `l` as a variable name is fine
        "indirect": [],
        "direct": ["Reg A (windows)"],
        "indirect_doc": [],
        "direct_doc": ["Doc B (windows)"],
    }


@yak_test()
async def test_plugin_dep_errors(yak: Yak) -> None:
    # Tests are explained in the file
    await yak.build("//test_errors:recursive_target_1")

    await yak.build("//test_errors:regular_a")

    result = await yak.uquery("deps(//test_errors:regular_b)")
    assert "//test_errors:toolchain" in result.stdout
    result = await expect_failure(yak.build("//test_errors:regular_b"))
    assert (
        "Plugin dep `root//test_errors:toolchain` is a toolchain rule" in result.stderr
    )

    result = await expect_failure(yak.build("//test_errors:wrong_plugin_kind"))
    assert "The rule did not declare that it uses plugins of kind A" in result.stderr


@yak_test()
async def test_repeated_insertion(yak: Yak) -> None:
    result = await yak.cquery(
        "-a", "yak.plugins", "//repeated_insertion:different_deps_alias"
    )
    assert {"Plugin": ["root//repeated_insertion:plugin"]} == list(
        json.loads(result.stdout).values()
    )[0]["yak.plugins"]


@yak_test()
async def test_visibility(yak: Yak) -> None:
    result = await expect_failure(yak.build("//visibility:missing_access"))
    assert (
        "`root//visibility/package:hidden` is not visible to `root//visibility:missing_access`"
        in result.stderr
    )

    await yak.build("//visibility:has_access")
