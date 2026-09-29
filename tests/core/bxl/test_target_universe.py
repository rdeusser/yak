# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_bxl_target_universe_keep_going_no_errors(yak: Yak) -> None:
    await yak.bxl(
        "//target_universe.bxl:target_universe_keep_going_no_errors",
    )


@yak_test()
async def test_bxl_target_universe_universe_target_set(yak: Yak) -> None:
    await yak.bxl(
        "//target_universe.bxl:target_universe_universe_target_set",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_with_errors(yak: Yak) -> None:
    await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_with_errors",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_list_input(yak: Yak) -> None:
    await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_list_input",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_target_set_input(yak: Yak) -> None:
    await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_target_set_input",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_mixed_list(yak: Yak) -> None:
    await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_mixed_list",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_all_fail(yak: Yak) -> None:
    await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_all_fail",
    )


@yak_test()
async def test_bxl_target_universe_keep_going_incompatible_target_set(
    yak: Yak,
) -> None:
    result = await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_target_set",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr


@yak_test()
async def test_bxl_target_universe_keep_going_incompatible_string_pattern(
    yak: Yak,
) -> None:
    result = await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_string_pattern",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr


@yak_test()
async def test_bxl_target_universe_keep_going_incompatible_list(
    yak: Yak,
) -> None:
    result = await yak.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_list",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr
