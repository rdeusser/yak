# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


async def _test_analysis_query_invalidation_impl(yak: Yak, name: str) -> None:
    linux = await yak.build_without_report(
        ":root", "-c", "test.configuration=linux", "--out=-"
    )
    macos = await yak.build_without_report(
        ":root", "-c", "test.configuration=macos", "--out=-"
    )

    golden(
        output=linux.stdout,
        rel_path=f"{name}/linux.txt.golden",
    )
    golden(
        output=macos.stdout,
        rel_path=f"{name}/macos.txt.golden",
    )

    # Mostly here to really be safe but in practice this fails with an
    # incompatible target earlier if we have a bug.
    assert "linux-select-dep" in linux.stdout
    assert "macos-select-dep" in macos.stdout


@yak_test(data_dir="analysis_query_invalidation")
async def test_analysis_query_invalidation_deps(yak: Yak) -> None:
    """
    Checks that a `deps()` analysis query sees the dependencies that a yakconfig change selects.
    """
    await _test_analysis_query_invalidation_impl(
        yak, name="analysis_query_invalidation"
    )


@yak_test(data_dir="analysis_query_deps")
async def test_analysis_query_deps(yak: Yak) -> None:
    deps = await yak.build_without_report(":deps", "--out=-")
    golden(
        output=deps.stdout,
        rel_path="analysis_query_deps/deps.txt.golden",
    )
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" in deps.stdout
    assert ":qux" in deps.stdout


@yak_test(data_dir="analysis_query_deps")
async def test_duplicate_analysis_query_expansions(yak: Yak) -> None:
    result = await yak.build_without_report(":duplicate_queries", "--out=-")
    fields = result.stdout.strip().split("|")
    assert len(fields) == 4
    assert all(fields)
    assert fields[0] == fields[2]
    assert fields[1] == fields[3]
    assert fields[0].endswith(":bar")


@yak_test(data_dir="analysis_query_deps")
async def test_analysis_query_deps_with_depth(yak: Yak) -> None:
    deps = await yak.build_without_report(":deps1", "--out=-")
    golden(output=deps.stdout, rel_path="analysis_query_deps/deps1.txt.golden")
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" in deps.stdout
    assert ":qux" not in deps.stdout


@yak_test(data_dir="analysis_query_deps")
async def test_analysis_query_target_deps(yak: Yak) -> None:
    deps = await yak.build_without_report(":target_deps", "--out=-")
    golden(
        output=deps.stdout,
        rel_path="analysis_query_deps/target_deps.txt.golden",
    )
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" not in deps.stdout
    assert ":qux" not in deps.stdout
