# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import (
    golden,
    GOLDEN_DIRECTORY,
    sanitize_build_report,
    sanitize_hashes,
)


def build_report_test(
    name: str,
    command: list[str],
    expect_error: bool = False,
) -> None:
    async def impl(yak: Yak, tmp_path: Path) -> None:
        report = tmp_path / "build-report.json"
        command.extend(["--build-report", str(report)])

        if expect_error:
            await expect_failure(yak.build(*command))
        else:
            await yak.build(*command)

        with open(report) as file:
            report = json.loads(file.read())

        sanitize_build_report(report)

        golden(
            output=sanitize_hashes(json.dumps(report, indent=2, sort_keys=True)),
            rel_path=GOLDEN_DIRECTORY + name + ".golden.json",
        )

    globals()[name] = impl

    return yak_test()(impl)


build_report_test(
    "test_build_modifiers_report",
    [
        "root//:target?root//:macos+root//:arm",
        "root//:target?root//:linux",
    ],
)

build_report_test(
    "test_build_modifiers_report_error_failures_includes_modifiers",
    ["root//fail:fail?root//:macos+root//:arm"],
    expect_error=True,
)

build_report_test(
    "test_build_modifiers_report_package_pattern",
    ["root//:?root//:macos"],
)

build_report_test(
    "test_build_modifiers_report_recursive_pattern",
    ["root//...?root//:macos"],
    expect_error=True,
)

build_report_test(
    "test_build_modifiers_report_ambiguous_pattern",
    [
        "root//subdir?root//:macos",
        "root//subdir:subdir?root//:linux",
        "root//subdir:subdir?root//:macos",
    ],
)

build_report_test(
    "test_build_modifiers_report_deduplication",
    [
        "root//:?root//:macos",
        "root//:target2?root//:linux",
        "root//:target?root//:macos",
    ],
)


@yak_test()
async def test_build_modifiers_that_lead_to_same_configured(yak: Yak) -> None:
    mac_first = "root//:target?root//:macos+root//:arm"
    arm_first = "root//:target?root//:arm+root//:macos"

    result = await yak.build(
        mac_first,
        arm_first,
        "-c",
        "yak.detailed_aggregated_metrics=true",
        "-c",
        "yak.log_configured_graph_sketch=true",
        "-c",
        "yak.log_configured_graph_unconfigured_sketch=true",
    )

    report = json.loads(result.stdout)

    mac_configuration = report["results"][mac_first]["configured"]
    arm_configuration = report["results"][arm_first]["configured"]

    assert mac_configuration == arm_configuration
