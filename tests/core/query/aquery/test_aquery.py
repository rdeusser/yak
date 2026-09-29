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


@yak_test()
async def test_target(yak: Yak) -> None:
    stdout = (await yak.aquery("//:test", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="target.golden.json",
    )


@yak_test()
async def test_all_outputs(yak: Yak) -> None:
    stdout = (await yak.aquery("all_outputs(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="all_outputs.golden.json",
    )


@yak_test()
async def test_all_actions(yak: Yak) -> None:
    stdout = (await yak.aquery("all_actions(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="all_actions.golden.json",
    )


@yak_test()
async def test_all_outputs_subtarget(yak: Yak) -> None:
    stdout = (
        await yak.aquery("all_outputs('//:test[sub]')", "-a", "identifier")
    ).stdout

    golden(
        output=stdout,
        rel_path="all_outputs_subtarget.golden.json",
    )


@yak_test()
async def test_filter(yak: Yak) -> None:
    stdout = (
        await yak.aquery(
            "attrfilter('identifier', 'other', all_actions('//:test[sub]'))",
            "-a",
            "identifier",
        )
    ).stdout

    golden(
        output=stdout,
        rel_path="filter.golden.json",
    )


@yak_test()
async def test_deps(yak: Yak) -> None:
    stdout = (await yak.aquery("deps(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="deps.golden.json",
    )


@yak_test()
async def test_deps_bounded(yak: Yak) -> None:
    stdout = (await yak.aquery("deps(//:test, 1)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="deps_bounded.golden.json",
    )


@yak_test()
async def test_rdeps(yak: Yak) -> None:
    # The universe contains an analysis node (from the target literal), which the
    # flattened-graph rdeps used to fail on with `Not an action`.
    stdout = (
        await yak.aquery("rdeps(deps(//:test), deps(//:test))", "-a", "identifier")
    ).stdout

    golden(
        output=stdout,
        rel_path="rdeps.golden.json",
    )


@yak_test()
async def test_bxl_aquery_target(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:target")).stdout
    golden(
        output=stdout,
        rel_path="bxl_target.golden.json",
    )


@yak_test()
async def test_bxl_aquery_all_outputs(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:all_outputs")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_outputs.golden.json",
    )


@yak_test()
async def test_bxl_aquery_all_actions(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:all_actions")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_actions.golden.json",
    )


@yak_test()
async def test_bxl_aquery_all_outputs_subtarget(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:all_outputs_subtarget")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_outputs_subtarget.golden.json",
    )


@yak_test()
async def test_bxl_aquery_attrfilter(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:attrfilter")).stdout

    golden(
        output=stdout,
        rel_path="bxl_filter.golden.json",
    )


@yak_test()
async def test_bxl_aquery_deps(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:deps")).stdout

    golden(
        output=stdout,
        rel_path="bxl_deps.golden.json",
    )


@yak_test()
async def test_bxl_aquery_eval(yak: Yak) -> None:
    stdout = (await yak.bxl("//:aquery.bxl:eval")).stdout

    golden(
        output=stdout,
        rel_path="bxl_eval.golden.json",
    )


@yak_test()
async def test_bxl_aquery_action_query_node(yak: Yak) -> None:
    await yak.bxl("//:aquery.bxl:action_query_node")


# Tests for bxl.Action support in aquery operations
@yak_test()
async def test_bxl_action_deps_0(yak: Yak) -> None:
    """Test passing a bxl.Action to aquery.deps() with depth=0 returns itself"""
    await yak.bxl("//:aquery.bxl:action_deps_0")


@yak_test()
async def test_bxl_action_deps(yak: Yak) -> None:
    """Test that you can isolate the deps of one action by itself"""
    await yak.bxl("//:aquery.bxl:action_deps")
