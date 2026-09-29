# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import os.path
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import is_running_on_windows


@yak_test()
async def test_log_show_invocation_record(yak: Yak, tmp_path: Path) -> None:
    mode_file = tmp_path / "mode"
    mode_file.write_text("-c\naa.bb=cc\n-c\ndd.ee=ff\n")

    # Any simple would do.
    await yak.uquery(f"@{mode_file}", "//:EEE")

    result = await yak.log("show")
    invocation = json.loads(result.stdout.splitlines()[0])
    command_line_args = invocation["command_line_args"]
    expanded_command_line_args = invocation["expanded_command_line_args"]
    assert f"@{mode_file}" in command_line_args
    assert f"@{mode_file}" not in expanded_command_line_args
    assert "aa.bb=cc" in expanded_command_line_args
    assert "aa.bb=cc" not in command_line_args


@yak_test(write_invocation_record=True)
async def test_log_size_logging(yak: Yak) -> None:
    res = await yak.cquery(
        "//:EEE",
    )

    out = await yak.log("last")
    path = out.stdout.strip()
    with open(path, "rb") as f:
        log_size_in_disk = len(f.read())

    logged_size = res.invocation_record()["compressed_event_log_size_bytes"]

    assert logged_size == log_size_in_disk


@yak_test()
async def test_last_log(yak: Yak) -> None:
    await yak.build("//:EEE")
    out = await yak.log("last")
    path = out.stdout.strip()
    assert os.path.exists(path)
    assert "/log/" in path or "\\log\\" in path
    out2 = await yak.log("path")
    assert path == out2.stdout.strip()


@yak_test()
async def test_last_log_all(yak: Yak) -> None:
    await yak.build("//:EEE")
    out = await yak.log("last", "--all")
    paths = list(out.stdout.splitlines())
    assert len(paths) > 0
    for path in paths:
        assert os.path.exists(path)
        assert "/log/" in path or "\\log\\" in path


@yak_test()
async def test_log_command_with_trace_id(yak: Yak, tmp_path: Path) -> None:
    build_file_path = tmp_path / "b"
    await yak.uquery("//:", f"--write-build-id={build_file_path}")
    build_id = build_file_path.read_text("utf-8").strip()
    await yak.log("show", f"--trace-id={build_id}")
    log = (await yak.log("show", f"--trace-id={build_id}")).stdout.strip().splitlines()
    # Check it looks like log.
    assert len(log) >= 1
    for line in log:
        json.loads(line)


@yak_test()
async def test_what_yak(yak: Yak, tmp_path: Path) -> None:
    mode_path = tmp_path / "mode"
    mode_path.write_text("-c\nxx.yy=zz\n")

    await yak.uquery("//:", f"@{mode_path}")

    out = await yak.log("what-cmd")
    assert "uquery //: " in out.stdout
    if not is_running_on_windows():
        # Path is quoted on Windows.
        assert f"uquery //: @{mode_path}" in out.stdout

    out = await yak.log("what-cmd", "--expand")
    assert "uquery //: -c" in out.stdout
