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
async def test_specific_target_success(yak: Yak) -> None:
    """Test unconfigured_targets_keep_going with a specific successful target."""
    await yak.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_specific_target_success",
    )


@yak_test()
async def test_recursive_pattern_success(yak: Yak) -> None:
    """Test unconfigured_targets_keep_going with a recursive pattern that includes only successful packages."""
    await yak.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_recursive_pattern_success",
    )


@yak_test()
async def test_recursive_pattern_mixed(yak: Yak) -> None:
    """Test unconfigured_targets_keep_going with a recursive pattern that includes both successful and failing packages."""
    await yak.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_recursive_pattern_mixed",
    )


@yak_test()
async def test_failing_package_only(yak: Yak) -> None:
    """Test unconfigured_targets_keep_going with a pattern that only matches a failing package."""
    await yak.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_failing_package_only",
    )


@yak_test()
async def test_specific_target_in_failing_package(yak: Yak) -> None:
    """Test unconfigured_targets_keep_going with a specific target in a failing package."""
    await yak.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_specific_target_in_failing_package",
    )
