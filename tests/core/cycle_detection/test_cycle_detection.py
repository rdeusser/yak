# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
from typing import Awaitable

from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException, YakResult
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


def check_load_cycle_stderr(stderr: str) -> None:
    # We're not sure which order these will appear.
    assert r"root//load_cycle/3.bzl ->" in stderr
    assert r"root//load_cycle/2.bzl ->" in stderr
    assert r"root//load_cycle/1.bzl ->" in stderr


def check_cfg_graph_cycle_stderr(stderr: str) -> None:
    # We're not sure which order these will appear.
    assert r"root//:cycle_bot (<unspecified>) ->" in stderr
    assert r"root//:cycle_mid (<unspecified>) ->" in stderr
    assert r"root//:cycle_top (<unspecified>) ->" in stderr


def check_cfg_toolchain_graph_cycle_stderr(stderr: str) -> None:
    # We're not sure which order these will appear.
    assert r"root//:toolchain_cycle_top" in stderr
    # toolchain_cycle_mid is the toolchain rule and it doesn't appear in the error. ideally we'd fix that, but
    # for performance/memory reasons we aggregate the exec_deps out of toolchain rules.
    # assert r"root//:toolchain_cycle_mid" in stderr
    assert r"root//:toolchain_cycle_bot" in stderr
    assert r"Resolving execution platform" in stderr


# It's better to fail a test than to hit our test timeout. When cycle detection is not working, yak will just hang. So wrap these in a timeout.
async def expect_cycle(
    process: Awaitable[YakResult],
) -> YakException:
    return await asyncio.wait_for(expect_failure(process), timeout=200)


@yak_test()
async def test_detect_load_cycle(yak: Yak) -> None:
    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.load=yes",
        ),
    )
    check_load_cycle_stderr(failure.stderr)


@yak_test()
async def test_detect_configured_graph_cycles(yak: Yak) -> None:
    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.cfg_graph=yes",
        ),
    )
    check_cfg_graph_cycle_stderr(failure.stderr)


@yak_test()
async def test_detect_configured_graph_cycles_on_recompute(yak: Yak) -> None:
    await yak.cquery("//:top")

    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.cfg_graph=yes",
        ),
    )

    check_cfg_graph_cycle_stderr(failure.stderr)


@yak_test()
async def test_detect_configured_graph_cycles_2(yak: Yak) -> None:
    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.cfg_toolchain=yes",
        ),
    )
    check_cfg_toolchain_graph_cycle_stderr(failure.stderr)


@yak_test()
async def test_more_recompute_cases(yak: Yak) -> None:
    await yak.cquery("//:top")

    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.load=yes",
        ),
    )
    check_load_cycle_stderr(failure.stderr)

    failure = await expect_cycle(
        yak.cquery(
            "//:top",
            "-c",
            "cycles.cfg_graph=yes",
        ),
    )
    check_cfg_graph_cycle_stderr(failure.stderr)
