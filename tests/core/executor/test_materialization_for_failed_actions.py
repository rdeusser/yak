# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events, json_get, random_string

HASH = r"[0-9a-fA-F]{16}"


async def check_materialize_inputs_for_failed_actions(yak: Yak) -> None:
    log = (await yak.log("show")).stdout.strip().splitlines()

    found_action_error = False
    found_materialize_failed_inputs_span = False

    for line in log:
        # Look for MaterializeFailedInputs ReStage event
        if "MaterializeFailedInputs" in line:
            found_materialize_failed_inputs_span = True

        # Inspect failed input
        materialized_inputs_for_failed = json_get(
            line,
            "Event",
            "data",
            "Instant",
            "data",
            "ActionError",
            "last_command",
            "details",
            "command_kind",
            "command",
            "RemoteCommand",
            "materialized_inputs_for_failed",
        )

        if materialized_inputs_for_failed:
            found_action_error = True
            assert len(materialized_inputs_for_failed) == 1
            input = materialized_inputs_for_failed[0]
            with open(Path(yak.cwd / input), "r") as materialized_input_path:
                contents = materialized_input_path.read()
                assert contents == "yay!"

    if not found_action_error:
        raise AssertionError("Did not find relevant ActionError")
    if not found_materialize_failed_inputs_span:
        raise AssertionError("Did not find relevant MaterializeFailedInputs span")


@pytest.mark.remote_execution
@yak_test(data_dir="materialize_inputs_for_failed_actions")
async def test_materialize_inputs_for_failed_actions(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--no-remote-cache",
            "--materialize-failed-inputs",
            "-c",
            f"test.cache_buster={random_string()}",
        ),
    )
    await check_materialize_inputs_for_failed_actions(yak)


@pytest.mark.remote_execution
@yak_test(data_dir="materialize_inputs_for_failed_actions")
async def test_materialize_inputs_for_failed_actions_content_hash(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--no-remote-cache",
            "--materialize-failed-inputs",
            "-c",
            f"test.cache_buster={random_string()}",
            "test.use_content_based_path=true",
        ),
    )
    await check_materialize_inputs_for_failed_actions(yak)


async def check_materialized_outputs_for_failed_action(yak: Yak) -> None:
    materialized = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "ActionError",
        "last_command",
        "details",
        "command_kind",
        "command",
        "RemoteCommand",
        "materialized_outputs_for_failed_actions",
    )

    if not materialized:
        raise AssertionError("No materialization for failed actions!")

    assert len(materialized) == 1 and len(materialized[0]) == 2

    out1 = materialized[0][0]
    with open(Path(yak.cwd / out1), "r") as materialized_input_path:
        contents = materialized_input_path.read()
        assert contents == "json"

    out2 = materialized[0][1]
    with open(Path(yak.cwd / out2), "r") as materialized_input_path:
        contents = materialized_input_path.read()
        assert contents == "txt"

    assert re.search(HASH, out1), "Expected hash in output path"
    assert re.search(HASH, out2), "Expected hash in output path"


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows"], data_dir="materialize_outputs_for_failed_actions")
async def test_materialize_outputs_for_failed_actions(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--materialize-failed-outputs",
        ),
    )
    await check_materialized_outputs_for_failed_action(yak)


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows"], data_dir="materialize_outputs_for_failed_actions")
async def test_materialize_outputs_for_failed_actions_content_hash(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--materialize-failed-outputs",
            "-c",
            "test.use_content_based_path=true",
        ),
    )
    await check_materialized_outputs_for_failed_action(yak)


@yak_test(data_dir="materialize_outputs_for_failed_actions")
async def test_undeclared_outputs_to_materialize_will_fail(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:undeclared_output",
            "--remote-only",
            "--no-remote-cache",
        ),
        stderr_regex="marked to be materialized on failure but is not declared as an output of the action",
    )


async def check_materialized_outputs_defined_by_run_action(yak: Yak) -> None:
    materialized = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "ActionError",
        "last_command",
        "details",
        "command_kind",
        "command",
        "RemoteCommand",
        "materialized_outputs_for_failed_actions",
    )

    if not materialized:
        raise AssertionError("No materialization for failed actions!")

    assert len(materialized) == 1
    assert len(materialized[0]) == 1

    out = materialized[0][0]
    with open(Path(yak.cwd / out), "r") as materialized:
        contents = materialized.read()
        assert contents == "json"

    assert re.search(HASH, out), "Expected hash in output path"


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows"], data_dir="materialize_outputs_for_failed_actions")
async def test_materialize_outputs_defined_by_run_action(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--no-remote-cache",
        ),
    )
    await check_materialized_outputs_defined_by_run_action(yak)


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows"], data_dir="materialize_outputs_for_failed_actions")
async def test_materialize_outputs_defined_by_run_action_content_hash(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.build(
            "//:action_fail",
            "--remote-only",
            "--no-remote-cache",
            "-c",
            "test.use_content_based_path=true",
        ),
    )
    await check_materialized_outputs_defined_by_run_action(yak)
