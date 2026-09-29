# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import typing

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


async def check_rule_type_names(
    yak: Yak, expected_rule_type_names: typing.List[typing.Optional[str]]
) -> None:
    rule_names = await filter_events(
        yak,
        "Result",
        "result",
        "build_response",
        "build_targets",
    )
    rule_names = rule_names[0]
    assert len(rule_names) == len(expected_rule_type_names)
    for actual, expected in zip(rule_names, expected_rule_type_names):
        if expected is not None:
            assert actual["target_rule_type_name"] == expected


@yak_test()
async def test_build_nested_subtargets(yak: Yak) -> None:
    await yak.build(
        "//:nested[sub][nested_sub]",
    )
    await check_rule_type_names(yak, ["nested_subtargets"])


@yak_test()
async def test_build_single_dep_touch(yak: Yak) -> None:
    await yak.build(
        "//:rule1",
    )
    await check_rule_type_names(yak, ["one"])


@yak_test()
async def test_build_two_out_of_order(yak: Yak) -> None:
    await yak.build(
        "//:rule1",
        "//:nested[sub][nested_sub]",
    )
    await check_rule_type_names(yak, ["nested_subtargets", "one"])


@yak_test()
async def test_build_rule_with_transition(yak: Yak) -> None:
    await yak.build(
        "//:a_writer_with_transition",
    )

    await check_rule_type_names(yak, ["three_with_transition"])


@yak_test()
async def test_build_all_in_target(yak: Yak) -> None:
    await yak.build(
        "//:",
    )
    await check_rule_type_names(
        yak,
        [
            "two",
            "three_with_transition",
            "nested_subtargets",
            "one",
            "one",
            "platform",
        ],
    )


@yak_test()
async def test_build_all_recursive(yak: Yak) -> None:
    await yak.build(
        "//...",
    )
    await check_rule_type_names(
        yak,
        [
            "two",
            "three_with_transition",
            "nested_subtargets",
            "one",
            "one",
            "platform",
        ],
    )
