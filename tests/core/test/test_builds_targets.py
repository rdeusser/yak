# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from typing import List

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


async def _check_build_and_skip_override(
    yak: Yak,
    target: str,
    skip_flag: str,
    config_args: List[str],
    failure_category: str,
) -> None:
    result = await expect_failure(yak.test(target, *config_args))
    assert failure_category in result.stderr

    await yak.test(target, *config_args, skip_flag)


@yak_test()
async def test_builds_targets_from_command_line_config(yak: Yak) -> None:
    await _check_build_and_skip_override(
        yak,
        "//:failing_default_info",
        "--skip-default-info",
        ["-c", "yak.test_builds_targets=true"],
        "failing_default_info",
    )
    await _check_build_and_skip_override(
        yak,
        "//:failing_run_info",
        "--skip-run-info",
        ["-c", "yak.test_builds_targets=true"],
        "failing_run_info",
    )


@yak_test()
async def test_explicit_build_flags_override_disabled_config(yak: Yak) -> None:
    default_result = await expect_failure(
        yak.test(
            "//:failing_default_info",
            "-c",
            "yak.test_builds_targets=false",
            "--build-default-info",
        )
    )
    assert "failing_default_info" in default_result.stderr

    run_result = await expect_failure(
        yak.test(
            "//:failing_run_info",
            "-c",
            "yak.test_builds_targets=false",
            "--build-run-info",
        )
    )
    assert "failing_run_info" in run_result.stderr


@yak_test()
async def test_builds_targets_from_included_config_file(yak: Yak) -> None:
    config_file = yak.cwd / "test_builds_targets_include.bcfg"

    await _check_build_and_skip_override(
        yak,
        "//:failing_default_info",
        "--skip-default-info",
        ["--config-file", str(config_file)],
        "failing_default_info",
    )
