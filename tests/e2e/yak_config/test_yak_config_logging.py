# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_yak_config_logging_disabled(yak: Yak) -> None:
    result = await yak.targets("//:")
    assert "starlark_log_yakconfig" not in result.stderr
    assert "starlark_log_all_yakconfigs" not in result.stderr


@yak_test()
async def test_yak_config_logging_enabled(yak: Yak) -> None:
    result = await yak.targets("//:", "--config", "yakconfig.log=test.read1")
    lines = [
        line
        for line in result.stderr.splitlines()
        if "starlark_log_yakconfig" in line
        # Unfortunately, "starlark_log_yakconfig" also shows up inside the stacktrace, so
        # try to exclude these lines here
        and "print(" not in line
    ]
    assert len(lines) == 1

    result = await yak.targets(
        "//:", "--config", "yakconfig.log=test.not_a_valid_yakconfig"
    )
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_yakconfig" in line
    ]
    assert len(lines) == 0


@yak_test()
async def test_yak_config_logging_enabled_json(yak: Yak) -> None:
    result = await yak.targets("//:", "--config", "yakconfig.log_json=test.read1")
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_yakconfig" in line
    ]
    assert len(lines) == 1, result.stderr
    # Terrible way to strip out the timestamp from the log line...
    read_config = json.loads(lines[0].split(maxsplit=1)[1].strip())[
        "starlark_log_yakconfig"
    ]
    assert read_config["cell"] == "root"

    result = await yak.targets(
        "//:", "--config", "yakconfig.log_json=test.not_a_valid_yakconfig"
    )
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_yakconfig" in line
    ]
    assert len(lines) == 0


@yak_test()
async def test_yak_config_logging_all_enabled(yak: Yak) -> None:
    result = await yak.targets(
        "//:",
        "--config",
        "yakconfig.log_all_in_json=true",
    )
    print(result.stderr)
    jsons = [
        # Terrible way to strip out the timestamp from the log line...
        json.loads(line.split(maxsplit=1)[1])["starlark_log_all_yakconfigs"]
        for line in result.stderr.splitlines()
        if '{"starlark_log_all_yakconfigs' in line
    ]
    filtered = [j for j in jsons if j["section"] == "test"]
    assert len(filtered) == 2
    for j in filtered:
        assert j["key"] in ["read1", "read2"]
        assert j["cell"] == "root"
