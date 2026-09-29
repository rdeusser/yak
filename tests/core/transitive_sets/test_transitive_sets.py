# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_transitive_sets(yak: Yak) -> None:
    rule = "//:bar"
    report = await yak.build(rule)
    out = report.get_build_report().output_for_target(rule)
    out = out.read_text()
    out = [line.strip() for line in out.strip().split("\n")]
    assert out == ["bar", "foo", "foo2", "foo1"]


@yak_test()
async def test_transitive_set_deduplication(yak: Yak) -> None:
    await yak.build("//:test_duplication")


@yak_test()
async def test_project_as_args_with_optional_args(yak: Yak) -> None:
    rule = "//:combined_optional_args"

    # TODO(ianc) Allow project_as_args to skip nodes that return None
    await expect_failure(
        yak.build(rule),
        stderr_regex="error: Expected `Artifact | CellPath | CellRoot | Label | OutputArtifact | ProjectRoot | ResolvedStringWithMacros | TaggedCommandLine | TargetLabel | TransitiveSetArgsProjection | WriteJsonCliArgs | cmd_args | str | RunInfo`, but got `NoneType (repr: None)`",
    )
    # report = await yak.build(rule)
    # out = report.get_build_report().output_for_target(rule)
    # out = out.read_text()
    # out = [line.strip() for line in out.strip().split("\n")]
    # assert out == ["combined_optional_args", "optional_args1"]


@yak_test()
async def test_project_as_json_with_optional_args(yak: Yak) -> None:
    rule = "//:combined_optional_json_args"

    report = await yak.build(rule)
    out = report.get_build_report().output_for_target(rule)
    out = out.read_text()
    out = json.loads(out)

    # TODO(ianc) remove the None
    assert out == ["combined_optional_json_args", "optional_json_args1", None]
