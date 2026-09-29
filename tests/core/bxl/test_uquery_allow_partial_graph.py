# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test

BXL = "//:allow_partial_graph.bxl:"


@yak_test()
async def test_deps_defaults_to_strict(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(BXL + "deps_default"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_deps_allow_partial_graph_skips_broken_edge(yak: Yak) -> None:
    await yak.bxl(BXL + "deps_partial")


@yak_test()
async def test_rdeps_defaults_to_strict(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(BXL + "rdeps_default"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_rdeps_allow_partial_graph_skips_broken_package(yak: Yak) -> None:
    await yak.bxl(BXL + "rdeps_partial")


@yak_test()
async def test_recursive_pattern_as_typed_arg_is_not_covered(yak: Yak) -> None:
    await yak.bxl(BXL + "rdeps_partial_typed_pattern")


@yak_test()
async def test_eval_recursive_defaults_to_strict(yak: Yak) -> None:
    await expect_failure(
        yak.bxl(BXL + "eval_recursive_default"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_eval_recursive_allow_partial_graph_skips_broken_package(
    yak: Yak,
) -> None:
    await yak.bxl(BXL + "eval_recursive_partial")


@yak_test()
async def test_explicit_missing_target_fails_with_allow_partial_graph(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.bxl(BXL + "explicit_missing_target"),
        stderr_regex="Unknown target `nonexistent`",
    )


@yak_test()
async def test_explicit_broken_target_fails_with_allow_partial_graph(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.bxl(BXL + "explicit_broken_target"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_lazy_deps_defaults_to_strict(yak: Yak) -> None:
    await yak.bxl(BXL + "lazy_deps_default")


@yak_test()
async def test_lazy_deps_allow_partial_graph_skips_broken_edge(yak: Yak) -> None:
    await yak.bxl(BXL + "lazy_deps_partial")


@yak_test()
async def test_lazy_eval_recursive_defaults_to_strict(yak: Yak) -> None:
    await yak.bxl(BXL + "lazy_eval_recursive_default")


@yak_test()
async def test_lazy_eval_recursive_allow_partial_graph_skips_broken_package(
    yak: Yak,
) -> None:
    await yak.bxl(BXL + "lazy_eval_recursive_partial")


@yak_test()
async def test_lazy_explicit_broken_target_fails_with_allow_partial_graph(
    yak: Yak,
) -> None:
    await yak.bxl(BXL + "lazy_explicit_broken_target")
