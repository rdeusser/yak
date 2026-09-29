# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden_replace_cfg_hash

"""
Generally we test for basic functionality of things working here and do
more extensive testing in the uquery tests.
"""


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test(data_dir="unsorted")
async def test_query_inputs(yak: Yak) -> None:
    result = await yak.cquery("""inputs(set(root//bin:the_binary //lib:file1))""")
    assert result.stdout == "bin/YAK.fixture\n"


@yak_test(data_dir="unsorted")
async def test_query_cell(yak: Yak) -> None:
    result = await yak.cquery("""//stuff:magic""", rel_cwd=Path("special"))
    assert (
        _replace_hash(result.stdout)
        == "special//stuff:magic (root//platforms:platform1#<HASH>)\n"
    )


@yak_test(data_dir="unsorted")
async def test_query_relative(yak: Yak) -> None:
    result = await yak.cquery("""...""", rel_cwd=Path("special"))
    assert (
        _replace_hash(result.stdout)
        == "special//stuff:magic (root//platforms:platform1#<HASH>)\n"
    )


@yak_test(data_dir="unsorted")
async def test_query_provider_names(yak: Yak) -> None:
    await expect_failure(
        yak.cquery("'root//bin:the_binary[provider_name]'"),
        stderr_regex="Expected a target pattern without providers",
    )

    await expect_failure(
        yak.cquery("'root//bin:the_binary#some_flavor'"),
        stderr_regex="Invalid target name `the_binary#some_flavor`",
    )


@yak_test(data_dir="unsorted")
async def test_query_print_provider_text(yak: Yak) -> None:
    out = await yak.cquery("%s", "root//bin:the_binary", "--show-providers")
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/query_print_provider_text.golden.txt",
    )


@yak_test(data_dir="unsorted")
async def test_query_print_provider_json(yak: Yak) -> None:
    out = await yak.cquery("%s", "root//bin:the_binary", "--show-providers", "--json")
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/query_print_provider_json.golden.json",
    )


@yak_test(data_dir="unsorted")
async def test_query_chunked_stream(yak: Yak) -> None:
    q = "deps(root//bin:the_binary)"
    result1 = await yak.cquery(q)
    await yak.kill()
    result2 = await yak.cquery(q, env={"YAK_DEBUG_RAWOUTPUT_CHUNK_SIZE": "5"})
    assert result1.stdout == result2.stdout


@yak_test(data_dir="unsorted")
async def test_attributes(yak: Yak) -> None:
    attrs_out = await yak.cquery(
        "--output-attribute",
        "yak\\..*",
        "--output-attribute",
        "srcs",
        "set(root//bin:the_binary //lib:file1)",
    )
    attrs_json_out = await yak.cquery(
        "--output-attribute",
        "yak\\..*",
        "--output-attribute",
        "srcs",
        "--json",
        "set(root//bin:the_binary //lib:file1)",
    )
    # specifying any attrs enables json output
    assert attrs_json_out.stdout == attrs_out.stdout
    attrs_json_out = json.loads(_replace_hash(attrs_json_out.stdout))
    assert {
        "root//bin:the_binary (root//platforms:platform1#<HASH>)": {
            "yak.deps": [
                "root//:data (root//platforms:platform1#<HASH>)",
                "root//lib:lib1 (root//platforms:platform1#<HASH>)",
                "root//lib:lib2 (root//platforms:platform1#<HASH>)",
                "root//lib:lib3 (root//platforms:platform1#<HASH>)",
                "root//:foo_toolchain (root//platforms:platform1#<HASH>)",
                "root//:bin (root//platforms:platform1#<HASH>)",
            ],
            "yak.execution_platform": "<legacy_global_exec_platform>",
            "yak.package": "root//bin:YAK.fixture",
            "yak.plugins": {},
            "yak.target_configuration": "root//platforms:platform1#<HASH>",
            "yak.type": "_foo_binary",
            "yak.oncall": None,
            "srcs": ["root//bin/YAK.fixture"],
        },
        "root//lib:file1 (root//platforms:platform1#<HASH>)": {
            "yak.deps": [],
            "yak.execution_platform": "<legacy_global_exec_platform>",
            "yak.package": "root//lib:YAK.fixture",
            "yak.plugins": {},
            "yak.target_configuration": "root//platforms:platform1#<HASH>",
            "yak.type": "_foo_genrule",
            "yak.oncall": None,
        },
    } == attrs_json_out


# Tests for "%Ss" uses
@yak_test(data_dir="unsorted")
async def test_args_as_set(yak: Yak) -> None:
    out = await yak.cquery("%Ss", "root//bin:the_binary", "//lib:file1")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary (root//platforms:platform1#<HASH>)\nroot//lib:file1 (root//platforms:platform1#<HASH>)\n"
    )


@yak_test(data_dir="unsorted")
async def test_multi_query(yak: Yak) -> None:
    out = await yak.cquery("%s", "root//bin:the_binary", "//lib:file1")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary (root//platforms:platform1#<HASH>)\nroot//lib:file1 (root//platforms:platform1#<HASH>)\n"
    )


@yak_test(data_dir="unsorted")
async def test_query_attrfilter(yak: Yak) -> None:
    out = await yak.uquery(
        "attrfilter(yak.package, 'root//bin:YAK.fixture',root//bin:the_binary)"
    )
    assert out.stdout.strip() == "root//bin:the_binary"


@yak_test(data_dir="multi_query_universe")
async def test_multi_query_universe(yak: Yak) -> None:
    out = await yak.cquery(
        "deps(%s)", "root//:macos-bin", "//:common-dep", "--output-format=json"
    )
    # `common-dep` is configured for linux, so it must not include `only-on-macos` target.
    #   Which would be the case if we constructed universe from all the queries together
    #   instead of separate universes for each query.
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="multi_query_universe/multi_query_universe.golden.json",
    )


@yak_test(data_dir="unsorted")
async def test_multi_query_print_provider_text(yak: Yak) -> None:
    out = await yak.cquery(
        "%s", "root//bin:the_binary", "//lib:lib1", "--show-providers"
    )
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/multi_query_print_provider_text.golden.txt",
    )


@yak_test(data_dir="unsorted")
async def test_multi_query_print_provider_json(yak: Yak) -> None:
    out = await yak.cquery(
        "%s", "root//bin:the_binary", "//lib:lib1", "--show-providers", "--json"
    )

    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/multi_query_print_provider_json.golden.json",
    )


@yak_test(data_dir="visibility")
async def test_visibility(yak: Yak) -> None:
    for good in [
        "self//:pass1",
        "self//:pass2",
        "self//:pass3",
        "self//:pass4",
    ]:
        out = await yak.cquery(good)
        assert good in out.stdout

    for bad in [
        "self//:fail1",
        "self//:fail2",
        "self//:fail3",
        "self//:fail4",
    ]:
        print(bad)
        failure = await expect_failure(yak.cquery(bad))
        assert "not visible to `%s`" % bad in failure.stderr


@yak_test(data_dir="testsof")
async def test_testsof(yak: Yak) -> None:
    out = await yak.cquery(
        "testsof(//:foo_lib)",
        "--target-platforms",
        "//:platform_default_tests",
    )

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" not in out.stdout
    assert "root//:foo_lib" not in out.stdout

    out = await yak.cquery(
        "testsof(//:foo_lib)",
        "--target-platforms",
        "//:platform_more_tests",
    )

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" in out.stdout
    assert "root//:foo_lib" not in out.stdout


# DICE currently may re-evaluate dead nodes ignoring errors, but it cannot ignore panics.
# Disabling execution platforms through a yakconfig used to cause such a panic,
# which made builds fail at random.
@yak_test(data_dir="toolchain_deps")
async def test_disabling_of_execution_platforms(yak: Yak) -> None:
    # Run these commands 10x such that a stress run of 10 on continuous CI would run these commands 100x.
    # If there is a regression then the stress run would for sure detect it.
    for _ in range(10):
        query = "deps(set(tests/...))"
        await yak.cquery(query)
        await yak.cquery(query, "-c", "build.execution_platforms=")


@yak_test(data_dir="deps_query")
async def test_declared_deps_query(yak: Yak) -> None:
    await expect_failure(
        yak.cquery(
            "root//:declared_deps",
        ),
        stderr_regex="Error parsing target pattern `\\$declared_deps`",
    )


# Tests for intersect and except operators on FileSet, TargetSet, and String types
# These tests verify the fix for https://github.com/facebook/yak/issues/1109
@yak_test(data_dir="set_operators")
async def test_cquery_fileset_intersect(yak: Yak) -> None:
    """Test FileSet intersect FileSet using inputs()."""
    result = await yak.cquery(
        """inputs(root//:lib_a) intersect inputs(root//:lib_b)"""
    )
    assert result.stdout == "common.txt\n"


@yak_test(data_dir="set_operators")
async def test_cquery_targetset_except(yak: Yak) -> None:
    """Test TargetSet except TargetSet using set()."""
    result = await yak.cquery(
        """set(root//:lib_a root//:app) except set(root//:app)"""
    )
    assert "root//:lib_a" in result.stdout
    assert "root//:app" not in result.stdout
