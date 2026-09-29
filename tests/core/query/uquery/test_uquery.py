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
from e2e_util.api.yak_result import YakResult
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test(data_dir="bxl_simple")
async def test_uquery_none(yak: Yak) -> None:
    await expect_failure(
        yak.uquery("""none"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        yak.uquery("""None"""),
        stderr_regex="expected value of type `targets`, got `None`:",
    )

    result = await yak.uquery(""":none""")
    assert result.stdout == "root//:none\n"

    result = await yak.uquery(""":None""")
    assert result.stdout == "root//:None\n"

    result = await yak.uquery("""':none'""")
    assert result.stdout == "root//:none\n"

    result = await yak.uquery("""':None'""")
    assert result.stdout == "root//:None\n"

    await expect_failure(
        yak.uquery("""set(none)"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        yak.uquery("""set(None)"""),
        # stderr_regex="expected value of type `targets`, got `None`:",
        stderr_regex="Error parsing target pattern `None`",
    )

    await expect_failure(
        yak.uquery("""set('none')"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        yak.uquery("""set('None')"""),
        stderr_regex="Error parsing target pattern `None`",
    )

    await expect_failure(
        yak.uquery("""filter('', none)"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        yak.uquery("""filter('', None)"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [1] `set: *target or file expression*`"
        ),
    )

    result = await yak.uquery("""filter(none, :none)""")
    assert result.stdout == "root//:none\n"

    await expect_failure(
        yak.uquery("""filter(None, :None)"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [0] `regex: *string*`"
        ),
    )

    result = await yak.uquery("""filter('none', :none)""")
    assert result.stdout == "root//:none\n"

    result = await yak.uquery("""filter('None', :None)""")
    assert result.stdout == "root//:None\n"

    result = await yak.uquery("""filter(none, ':none')""")
    assert result.stdout == "root//:none\n"

    await expect_failure(
        yak.uquery("""filter(None, ':None')"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [0] `regex: *string*`"
        ),
    )

    result = await yak.uquery("""filter('none', ':none')""")
    assert result.stdout == "root//:none\n"

    result = await yak.uquery("""filter('None', ':None')""")
    assert result.stdout == "root//:None\n"

    await expect_failure(
        yak.uquery("""none()"""),
        stderr_regex="unknown function `none`:",
    )
    await expect_failure(
        yak.uquery("""None()"""),
        stderr_regex="in Eof",
    )


@yak_test(data_dir="bxl_simple")
async def test_uquery_inputs(yak: Yak) -> None:
    result = await yak.uquery("""inputs(set(root//bin:the_binary //lib:file1))""")
    assert result.stdout == "bin/YAK.fixture\n"

    result = await yak.uquery("""inputs(set())""")
    assert result.stdout == ""


@yak_test(data_dir="bxl_simple")
async def test_uquery_union(yak: Yak) -> None:
    result = await yak.uquery("""deps(root//lib:lib1) + set(root//data:data)""")
    assert result.stdout == "root//lib:file1\nroot//lib:lib1\nroot//data:data\n"

    result = await yak.uquery(
        """buildfile(root//bin:the_binary) + inputs(deps(root//lib:lib1))"""
    )
    assert result.stdout == "bin/YAK.fixture\nlib/YAK.fixture\n"

    result = await yak.uquery("""'root//bin:the_binary' + set(root//data:data)""")
    assert result.stdout == "root//bin:the_binary\nroot//data:data\n"


@yak_test(data_dir="bxl_simple")
async def test_uquery_owner(yak: Yak) -> None:
    result = await yak.uquery("""owner(bin/YAK.fixture)""")
    assert result.stdout == "root//bin:the_binary\n"

    result = await yak.uquery("""owner(data/yak/build/data.file)""")
    assert result.stdout == "root//data:data\n"

    # there's no buildfile in the root of the special yak, make sure that works
    result = await yak.uquery("""owner(special/file)""")
    assert "No owner" in result.stderr
    assert result.stdout == ""

    # there's a buildfile here, but no target owns the file
    result = await yak.uquery("""owner(.yakconfig)""")
    assert "No owner" in result.stderr
    assert result.stdout == ""

    result = await yak.uquery(
        """owner(../data/yak/build/data.file)""", rel_cwd=Path("special")
    )
    assert result.stdout == "root//data:data\n"

    result = await yak.uquery("""owner(root//bin/YAK.fixture)""")
    assert result.stdout == "root//bin:the_binary\n"


@yak_test(data_dir="bxl_simple")
async def test_query_owner_with_explicit_package_boundary_violation(yak: Yak) -> None:
    result = await yak.uquery("""owner(package_boundary_violation/bin)""")
    assert "root//package_boundary_violation:bin" in result.stdout
    assert "root//:package_boundary_violation" in result.stdout


@yak_test(data_dir="bxl_simple", allow_soft_errors=True)
async def test_uquery_buildfile(yak: Yak) -> None:
    result = await yak.uquery("""buildfile(root//bin:the_binary)""")
    assert result.stdout == "bin/YAK.fixture\n"

    result = await yak.uquery("""buildfile(root//bin: + root//data:)""")
    assert result.stdout == "bin/YAK.fixture\ndata/YAK.fixture\n"

    result = await yak.uquery(
        """buildfile(owner(../data/yak/build/data.file))""", rel_cwd=Path("special")
    )
    assert result.stdout == "data/YAK.fixture\n"


@yak_test(data_dir="bxl_simple")
async def test_uquery_targets_in_buildfile(yak: Yak) -> None:
    result = await yak.uquery("""targets_in_buildfile(bin/YAK.fixture)""")
    assert (
        result.stdout
        == "\n".join(
            [
                "root//bin:setting",
                "root//bin:my_config",
                "root//bin:my_platform",
                "root//bin:the_binary",
                "root//bin:the_binary_with_dir_srcs",
                "root//bin:platform",
            ]
        )
        + "\n"
    )


@yak_test(data_dir="bxl_simple")
async def test_query_configuration_deps(yak: Yak) -> None:
    result = await yak.uquery(
        """deps(root//bin:the_binary, 1, configuration_deps())"""
    )
    assert "root//bin:my_config" in result.stdout


@yak_test(data_dir="bxl_simple")
async def test_deps(yak: Yak) -> None:
    result = await yak.uquery("""deps(root//bin:the_binary)""")
    assert (
        result.stdout
        == "\n".join(
            [
                "root//:foo_toolchain",
                "root//:bin",
                "root//lib:file3",
                "root//lib:lib3",
                "root//lib:file2",
                "root//lib:lib2",
                "root//lib:file1",
                "root//lib:lib1",
                "root//:genrule_binary",
                "root//:data",
                "root//bin:the_binary",
            ]
        )
        + "\n"
    )

    target_deps_expr = """deps(root//bin:the_binary, 100, target_deps())"""

    result = await yak.uquery(target_deps_expr)
    assert (
        result.stdout
        == "\n".join(
            [
                "root//bin:the_binary",
                "root//:data",
                "root//lib:lib1",
                "root//lib:lib2",
                "root//lib:lib3",
                "root//lib:file1",
                "root//lib:file2",
                "root//lib:file3",
            ]
        )
        + "\n"
    )

    # this is a little subtle, query's deps() function always forms a graph
    # with the nodes themselves so we subtract them out. It's not quite right
    # if a node in the graph of target deps were to have an exec dep on another.
    result = await yak.uquery(
        "deps({}, 1, exec_deps()) - {}".format(target_deps_expr, target_deps_expr)
    )
    assert (
        result.stdout
        == "\n".join(
            [
                "root//:foo_toolchain",
                "root//:bin",
                "root//:genrule_binary",
            ]
        )
        + "\n"
    )


@yak_test(data_dir="bxl_simple")
async def test_uquery_cell(yak: Yak) -> None:
    result = await yak.uquery("""//stuff:magic""", rel_cwd=Path("special"))
    assert result.stdout == "special//stuff:magic\n"


@yak_test(data_dir="bxl_simple")
async def test_uquery_relative(yak: Yak) -> None:
    result = await yak.uquery("""...""", rel_cwd=Path("special"))
    assert result.stdout == "special//stuff:magic\n"
    result = await yak.uquery("""...""", rel_cwd=Path("bin"))
    assert "root//bin:the_binary\n" in result.stdout


@yak_test(data_dir="bxl_simple")
async def test_uquery_provider_names(yak: Yak) -> None:
    await expect_failure(
        yak.uquery("'root//bin:the_binary[provider_name]'"),
        stderr_regex="Expected a target pattern without providers",
    )

    await expect_failure(
        yak.uquery("'root//bin:the_binary#some_flavor'"),
        stderr_regex="Invalid target name `the_binary#some_flavor`",
    )


@yak_test(data_dir="bxl_simple")
async def test_query_filter(yak: Yak) -> None:
    # Test uquery/cquery on target and file sets
    out = await yak.uquery("filter('the_binary$', root//...)")
    assert out.stdout == "root//bin:the_binary\n"
    out = await yak.cquery("filter('the_binary\\w', root//...)")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary_with_dir_srcs (root//platforms:platform1#<HASH>)\n"
    )
    out = await yak.uquery("filter('fixture$', inputs(root//bin:the_binary))")
    assert out.stdout == "bin/YAK.fixture\n"
    out = await yak.cquery("filter('fixture$', inputs(root//bin:the_binary))")
    assert out.stdout == "bin/YAK.fixture\n"


@yak_test(data_dir="bxl_simple")
async def test_attributes(yak: Yak) -> None:
    out = await yak.uquery("set(root//bin:the_binary //lib:file1)")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    json_out = await yak.uquery("--json", "set(root//bin:the_binary //lib:file1)")
    json_out = json.loads(json_out.stdout)
    assert ["root//bin:the_binary", "root//lib:file1"] == json_out

    attrs_out = await yak.uquery(
        "--output-attribute",
        "yak\\..*",
        "--output-attribute",
        "srcs",
        "--output-attribute",
        "deps",
        "set(root//bin:the_binary //lib:file1)",
    )
    attrs_json_out = await yak.uquery(
        "--output-attribute",
        "yak\\..*",
        "--output-attribute",
        "srcs",
        "--output-attribute",
        "deps",
        "--json",
        "set(root//bin:the_binary //lib:file1)",
    )
    # specifying any attrs enables json output
    assert attrs_json_out.stdout == attrs_out.stdout
    attrs_json_out = json.loads(attrs_json_out.stdout)
    assert {
        "root//bin:the_binary": {
            "yak.deps": [
                "root//:data",
                "root//lib:lib1",
                "root//lib:lib2",
                "root//lib:lib3",
                "root//:foo_toolchain",
                "root//:bin",
            ],
            "yak.package": "root//bin:YAK.fixture",
            "yak.tree_modifiers": ["cfg//os:linux"],
            "yak.type": "_foo_binary",
            "yak.configuration_deps": ["root//bin:my_platform", "root//bin:my_config"],
            "yak.oncall": None,
            "deps": ["root//lib:lib1", "root//lib:lib2", "root//lib:lib3"],
            "srcs": ["root//bin/YAK.fixture"],
        },
        "root//lib:file1": {
            "yak.deps": [],
            "yak.package": "root//lib:YAK.fixture",
            "yak.tree_modifiers": ["cfg//os:linux"],
            "yak.type": "_foo_genrule",
            "yak.configuration_deps": ["root//platforms:platform1"],
            "yak.oncall": None,
        },
    } == attrs_json_out


@yak_test(data_dir="bxl_simple")
async def test_dot(yak: Yak) -> None:
    out = await yak.uquery("--dot", "deps(root//bin:the_binary, 100, target_deps())")
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/deps.golden")

    out = await yak.uquery(
        "--dot",
        "--output-attribute=name",
        "--output-attribute=^deps",
        "--output-attribute=cmd",
        "deps(root//bin:the_binary, 100, target_deps()) - //platforms:",
    )
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/attrs.golden")

    out = await yak.uquery(
        "--dot",
        "deps(root//bin:the_binary, 100, target_deps()) - set(//lib: //platforms:)",
    )
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/subgraph.golden")


@yak_test(data_dir="bxl_simple")
async def test_dot_compact(yak: Yak) -> None:
    out = await yak.uquery(
        "--dot-compact", "deps(root//bin:the_binary, 100, target_deps())"
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/deps.golden",
    )

    out = await yak.uquery(
        "--dot-compact",
        "--output-attribute=name",
        "--output-attribute=^deps",
        "--output-attribute=cmd",
        "deps(root//bin:the_binary, 100, target_deps()) - //platforms:",
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/attrs.golden",
    )

    out = await yak.uquery(
        "--dot-compact",
        "deps(root//bin:the_binary, 100, target_deps()) - set(//lib: //platforms:)",
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/subgraph.golden",
    )


# Tests for "%Ss" uses
@yak_test(data_dir="bxl_simple")
async def test_args_as_set(yak: Yak) -> None:
    out = await yak.uquery("%Ss", "root//bin:the_binary", "//lib:file1")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    result = await yak.uquery("--json", "%Ss", "root//bin:the_binary", "//lib:file1")
    json_out = json.loads(result.stdout)
    assert json_out == ["root//bin:the_binary", "root//lib:file1"]


@yak_test(data_dir="bxl_simple")
async def test_multi_uquery(yak: Yak) -> None:
    out = await yak.uquery("%s", "root//bin:the_binary", "//lib:file1")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    result = await yak.uquery(
        "owner(%s)", "bin/YAK.fixture", "data/yak/build/data.file"
    )
    assert result.stdout == "root//bin:the_binary\nroot//data:data\n"

    result = await yak.uquery(
        "--json", "owner(%s)", "bin/YAK.fixture", "data/yak/build/data.file"
    )
    json_out = json.loads(result.stdout)

    assert json_out == {
        "bin/YAK.fixture": ["root//bin:the_binary"],
        "data/yak/build/data.file": ["root//data:data"],
    }

    # A multi-query with --output-attribute merges its results.
    result = await yak.uquery(
        "--json",
        "--output-attribute=name",
        "owner(%s)",
        "bin/YAK.fixture",
        "data/yak/build/data.file",
    )
    json_out = json.loads(result.stdout)

    assert json_out == {
        "root//bin:the_binary": {"name": "the_binary"},
        "root//data:data": {"name": "data"},
    }

    # test a case where the query for one arg fails. The process should exit with a non-zero code, but
    # the produced output should be valid json with an appropriate error indicator.
    failure = await expect_failure(
        yak.uquery("--json", "inputs(%s)", "//data:data", "xyz")
    )
    json_out = json.loads(failure.stdout)
    assert "$error" in json_out["xyz"]
    assert json_out["//data:data"] == ["data/yak/build/data.file"]

    # Test where the parameter is not a literal, but a query fragment
    out = await yak.uquery("%s", "deps(root//lib:lib1)")
    assert out.stdout == "root//lib:file1\nroot//lib:lib1\n"

    out = await yak.uquery("owner(%s)", "inputs(root//bin:the_binary)")
    assert out.stdout == "root//bin:the_binary\n"

    out = await yak.uquery("owner(%s)", "data/yak/build/data.file")
    assert out.stdout == "root//data:data\n"

    # We'd really prefer this to be an error
    out = await yak.uquery("owner(%s", "data/yak/build/data.file)")
    assert out.stdout == "root//data:data\n"


@yak_test(data_dir="testsof")
async def test_testsof(yak: Yak) -> None:
    out = await yak.uquery("testsof(//:foo_lib)")

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" in out.stdout
    assert "root//:foo_lib" not in out.stdout


@yak_test(data_dir="directory_sources")
async def test_directory_source(yak: Yak) -> None:
    await yak.build(":a_file")
    await yak.build(":a_dir")

    result = await yak.query("owner(dir/file1.txt)")
    assert result.stdout == "root//:a_dir\n"
    result = await yak.query("inputs(:a_dir)")
    assert (
        result.stdout == "dir/file1.txt\ndir/subdir/file2.txt\ndir/subdir/file3.txt\n"
    )

    # Can't reference files that don't exist
    await expect_failure(
        yak.build("does_not_exist:"),
        stderr_regex="Source file `does_not_exist` does not exist as a member of package",
    )

    # Want to make sure we can't do a package boundary violation
    # Currently these are soft errors
    await expect_failure(
        yak.build("subpackage:"),
        stderr_regex="Source file `subpackage` does not exist as a member of package",
    )

    await expect_failure(
        yak.build("dir_with_subpackage"),
        stderr_regex="may not cover any subpackages, but includes subpackage `dir_with_subpackage/subpackage`.",
    )


@yak_test(data_dir="oncall")
async def test_oncall(yak: Yak) -> None:
    out = await yak.uquery("//:foo", "--output-attribute=oncall")
    assert '"magic"' in out.stdout
    out = await yak.cquery("//:bar", "--output-attribute=oncall")
    assert '"magic"' in out.stdout


@yak_test(data_dir="oncall")
async def test_output_all_attributes(yak: Yak) -> None:
    def contains(out: YakResult, want: list[str], notwant: list[str]) -> None:
        x = json.loads(out.stdout)["root//:foo"]
        for w in want:
            assert w in x
        for w in notwant:
            assert w not in x

    out = await yak.uquery("//:foo", "--output-all-attributes", "--json")
    contains(
        out,
        [
            "yak.type",
            "name",
            "yak.oncall",
            "yak.package",
            "yak.configuration_deps",
            "yak.deps",
            "visibility",
        ],
        ["madeup"],
    )
    out = await yak.uquery("//:foo", "--output-basic-attributes", "--json")
    contains(
        out,
        ["yak.type", "name", "yak.package", "visibility"],
        ["yak.oncall", "yak.configuration_deps"],
    )


@yak_test(data_dir="bxl_simple")
async def test_output_format_starlark_golden(yak: Yak) -> None:
    result = await yak.uquery(
        "--output-format=starlark",
        "--stack",
        "//lib:",
    )

    golden(
        output=result.stdout,
        rel_path="output_starlark.golden.out",
    )


@yak_test(data_dir="bxl_simple")
async def test_uquery_rdeps(yak: Yak) -> None:
    result = await yak.query("""rdeps(root//bin:the_binary, //lib:file1)""")
    assert result.stdout == "root//bin:the_binary\nroot//lib:lib1\nroot//lib:file1\n"

    result = await yak.query("""rdeps(root//bin:the_binary, //lib:file1, 0)""")
    assert result.stdout == "root//lib:file1\n"

    result = await yak.query("""rdeps(root//bin:the_binary, //lib:file1, 1)""")
    assert result.stdout == "root//lib:lib1\nroot//lib:file1\n"

    result = await yak.query("""rdeps(root//bin:the_binary, //lib:file1, 100)""")
    assert result.stdout == "root//bin:the_binary\nroot//lib:lib1\nroot//lib:file1\n"


@yak_test(data_dir="bxl_simple")
async def test_query_attrfilter_special_attribute(yak: Yak) -> None:
    out = await yak.uquery(
        "attrfilter(yak.package, 'root//bin:YAK.fixture',root//bin:the_binary)"
    )
    assert out.stdout.strip() == "root//bin:the_binary"


# Tests for intersect and except operators on FileSet, TargetSet, and String types
# These tests verify the fix for https://github.com/facebook/yak/issues/1109
@yak_test(data_dir="set_operators")
async def test_uquery_fileset_intersect(yak: Yak) -> None:
    """Test FileSet intersect FileSet using inputs()."""
    result = await yak.uquery(
        """inputs(root//:lib_a) intersect inputs(root//:lib_b)"""
    )
    assert result.stdout == "common.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_fileset_except(yak: Yak) -> None:
    """Test FileSet except FileSet using inputs()."""
    result = await yak.uquery("""inputs(root//:lib_a) except inputs(root//:lib_b)""")
    assert result.stdout == "lib_a.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_fileset_intersect_string(yak: Yak) -> None:
    """Test FileSet intersect String."""
    result = await yak.uquery("""inputs(root//:lib_a) intersect "common.txt" """)
    assert result.stdout == "common.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_fileset_except_string(yak: Yak) -> None:
    """Test FileSet except String."""
    result = await yak.uquery("""inputs(root//:lib_a) except "common.txt" """)
    assert result.stdout == "lib_a.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_intersect_fileset(yak: Yak) -> None:
    """Test String intersect FileSet."""
    result = await yak.uquery(""" "common.txt" intersect inputs(root//:lib_a)""")
    assert result.stdout == "common.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_except_fileset(yak: Yak) -> None:
    """Test String except FileSet (string not in fileset)."""
    result = await yak.uquery(""" "lib_a.txt" except inputs(root//:lib_b)""")
    assert result.stdout == "lib_a.txt\n"


@yak_test(data_dir="set_operators")
async def test_uquery_targetset_intersect(yak: Yak) -> None:
    """Test TargetSet intersect TargetSet using set()."""
    result = await yak.uquery(
        """set(root//:lib_a root//:app) intersect set(root//:lib_b root//:app)"""
    )
    assert result.stdout == "root//:app\n"


@yak_test(data_dir="set_operators")
async def test_uquery_targetset_except(yak: Yak) -> None:
    """Test TargetSet except TargetSet using set()."""
    result = await yak.uquery(
        """set(root//:lib_a root//:app) except set(root//:app)"""
    )
    assert result.stdout == "root//:lib_a\n"


@yak_test(data_dir="set_operators")
async def test_uquery_targetset_intersect_string(yak: Yak) -> None:
    """Test TargetSet intersect String."""
    result = await yak.uquery(
        """set(root//:lib_a root//:app) intersect "root//:lib_a" """
    )
    assert result.stdout == "root//:lib_a\n"


@yak_test(data_dir="set_operators")
async def test_uquery_targetset_except_string(yak: Yak) -> None:
    """Test TargetSet except String."""
    result = await yak.uquery("""set(root//:lib_a root//:app) except "root//:app" """)
    assert result.stdout == "root//:lib_a\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_intersect_targetset(yak: Yak) -> None:
    """Test String intersect TargetSet."""
    result = await yak.uquery(
        """ "root//:lib_a" intersect set(root//:lib_a root//:app)"""
    )
    assert result.stdout == "root//:lib_a\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_except_targetset(yak: Yak) -> None:
    """Test String except TargetSet (string not in targetset)."""
    result = await yak.uquery(
        """ "root//:app" except set(root//:lib_a root//:lib_b)"""
    )
    assert result.stdout == "root//:app\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_intersect_string(yak: Yak) -> None:
    """Test String intersect String for targets."""
    result = await yak.uquery(""" "root//:lib_a" intersect "root//:lib_a" """)
    assert result.stdout == "root//:lib_a\n"


@yak_test(data_dir="set_operators")
async def test_uquery_string_except_string(yak: Yak) -> None:
    """Test String except String (different targets)."""
    result = await yak.uquery(""" "root//:app" except "root//:lib_a" """)
    assert result.stdout == "root//:app\n"
