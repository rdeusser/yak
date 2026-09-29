# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import os
import re
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import (
    golden,
    sanitize_build_report,
    sanitize_hashes,
)


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


YAK_OUT_ROOT_REL_PATH = "yak-out/v2/art/root"


@pytest.mark.remote_execution
@yak_test()
async def test_bxl_ensure_no_materialization(yak: Yak) -> None:
    result = await yak.bxl(
        "//no_materialization_bxl_build/remote_text.bxl:ensure",
        "--materializations=none",
    )

    [output] = result.stdout.splitlines()
    assert os.path.exists(yak.cwd / Path(output)) is False

    result = await yak.bxl(
        "//no_materialization_bxl_build/remote_text.bxl:ensure",
    )

    [output] = result.stdout.splitlines()
    assert os.path.exists(yak.cwd / Path(output)) is True


@yak_test()
async def test_bxl_ensure(yak: Yak) -> None:
    result = await yak.bxl(
        "//ensure.bxl:ensure_build_result_test",
        "--",
        "--target",
        ":buildable",
    )

    outputs = json.loads(result.stdout)
    [yak_out] = [v for (k, v) in outputs.items() if k.startswith("root//:buildable")][
        0
    ]
    assert (yak.cwd / Path(yak_out)).read_text() == "abcd"

    result = await yak.bxl(
        "//ensure.bxl:ensure_cmd_line_test",
    )

    lines = sorted(result.stdout.splitlines())
    assert (yak.cwd / Path(lines[0])).read_text() == "run_info_out"
    assert (yak.cwd / Path(lines[1])).read_text() == "target_with_tset\n"
    assert (yak.cwd / Path(lines[2])).read_text() == "tset1\n"
    assert (yak.cwd / Path(lines[3])).read_text() == "tset2\n"
    assert (yak.cwd / Path(lines[4])).read_text() == "tset3\n"

    result = await yak.bxl(
        "//ensure.bxl:ensure_cmd_line_json_output",
    )

    json_array = sorted(json.loads(result.stdout))
    assert "target_with_tset" in json_array[0]
    assert "tset1" in json_array[1]
    assert "tset2" in json_array[2]
    assert "tset3" in json_array[3]


@yak_test(skip_for_os=["windows"])
async def test_bxl_artifact_path(yak: Yak) -> None:
    result = await yak.bxl(
        "//artifacts.bxl:artifact_path_test",
    )

    outputs = json.loads(result.stdout)

    assert outputs["source_artifact"] == "<source artifact artifacts/DATA>"
    # The project relative path of the source artifact
    assert outputs["source_artifact_project_rel_path"] == "artifacts/DATA"

    # Abs path for the source artifact. The path should exist on the filesystem.
    assert outputs["source_artifact_abs_path"] == str(yak.cwd / Path("artifacts/DATA"))
    assert (
        os.path.exists((yak.cwd / Path(outputs["source_artifact_abs_path"]))) is True
    )

    assert (
        "build artifact foo.txt bound to root//artifacts:with_build_artifact"
        in outputs["build_artifact"]
    )

    prefix = YAK_OUT_ROOT_REL_PATH + "/"

    # The project relative path to the yak-out directory with the output
    assert outputs["build_artifact_project_rel_path"].startswith(prefix)
    assert (
        "/artifacts/__with_build_artifact__/foo.txt"
        in outputs["build_artifact_project_rel_path"]
    )

    # Abs path for the build artifact. Path should not exist on the filesystem since it's not materialized.
    assert outputs["build_artifact_abs_path"] == str(
        yak.cwd / Path(outputs["build_artifact_project_rel_path"])
    )

    assert (
        os.path.exists((yak.cwd / Path(outputs["build_artifact_abs_path"]))) is False
    )


@yak_test(skip_for_os=["windows"])
async def test_bxl_artifact_path_cmd_args(yak: Yak) -> None:
    result = await yak.bxl(
        "//artifacts.bxl:cmd_args_artifact_path_test",
    )

    outputs = json.loads(result.stdout)
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__target_with_outputs__/run_info_out",
        outputs["target_with_outputs_rel_paths"][0],
        False,
    )

    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__target_with_outputs__/run_info_out",
        outputs["target_with_outputs_abs_paths"][0],
        True,
    )

    assert len(outputs["target_with_tset_rel_paths"]) == 4

    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__target_with_tset__/out.txt",
        outputs["target_with_tset_rel_paths"][0],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset1__/out.txt",
        outputs["target_with_tset_rel_paths"][1],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset2__/out.txt",
        outputs["target_with_tset_rel_paths"][2],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset3__/out.txt",
        outputs["target_with_tset_rel_paths"][3],
        False,
    )

    assert len(outputs["target_with_tset_abs_paths"]) == 4

    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__target_with_tset__/out.txt",
        outputs["target_with_tset_abs_paths"][0],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset1__/out.txt",
        outputs["target_with_tset_abs_paths"][1],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset2__/out.txt",
        outputs["target_with_tset_abs_paths"][2],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        yak,
        "kind/__tset3__/out.txt",
        outputs["target_with_tset_abs_paths"][3],
        True,
    )


def _test_bxl_artifact_path_cmd_args_helper(
    yak: Yak, part_to_validate: str, full_path: str, is_abs: bool
) -> None:
    assert YAK_OUT_ROOT_REL_PATH in full_path
    assert part_to_validate in full_path
    if is_abs:
        assert str((yak.cwd / Path(YAK_OUT_ROOT_REL_PATH))) in full_path
        assert os.path.exists(full_path) is False
    else:
        assert str(yak.cwd) not in full_path
        assert os.path.exists((yak.cwd / Path(full_path))) is False


@yak_test(allow_soft_errors=True, skip_for_os=["darwin", "windows"])
async def test_bxl_ensure_failures(yak: Yak, tmp_path: Path) -> None:
    """Test that BXL fails when trying to ensure a failed build artifact."""
    report = tmp_path / "build-report.json"

    await expect_failure(
        yak.bxl(
            "//ensure.bxl:ensure_failures",
            "--build-report",
            str(report),
        ),
    )

    with open(report) as f:
        build_report = json.loads(f.read())

    sanitize_build_report(build_report)

    golden(
        output=sanitize_hashes(json.dumps(build_report, indent=2, sort_keys=True)),
        rel_path="fixtures/test_bxl_ensure_failures.golden.json",
    )
