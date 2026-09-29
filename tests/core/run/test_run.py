# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import subprocess
from pathlib import Path
from typing import List

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_run_executable(yak: Yak) -> None:
    result = await yak.run("root//:print_hello")
    assert result.stdout.strip() == "hello"


@yak_test(skip_for_os=["windows"])
async def test_emit_shell(yak: Yak) -> None:
    result = await yak.run(
        "root//:print_hello",
        "--emit-shell",
    )

    out = subprocess.check_output(result.stdout, shell=True, encoding="utf-8")
    assert out.strip() == "hello"


@yak_test()
async def test_command_args_file(yak: Yak, tmp_path: Path) -> None:
    args_file = tmp_path / "command.json"
    await yak.run(
        "root//:echo_args",
        f"--command-args-file={args_file}",
        "--",
        "a",
        "b",
    )
    command = json.loads(args_file.read_text(encoding="utf-8"))
    assert sorted(command) == ["argv", "envp", "path"]
    assert command["path"] == command["argv"][0]
    out = subprocess.check_output(
        command["argv"], env=command["envp"], encoding="utf-8"
    )
    assert out.strip() == "a b"


@yak_test(write_invocation_record=True)
async def test_run_non_executable_fails(yak: Yak) -> None:
    res = await expect_failure(
        yak.run(
            "root//:no_run_info",
        ),
        stderr_regex=r"Target `[^`]+` is not a binary rule \(only binary rules can be `run`\)",
    )

    record = res.invocation_record()
    [error] = record["errors"]

    assert error["category_key"] == "RunCommandError::NonBinaryRule"
    assert error["category"] == "USER"


@yak_test(write_invocation_record=True)
async def test_run_exit_result(yak: Yak) -> None:
    res = await yak.run(
        "root//:print_hello",
    )
    record = res.invocation_record()
    assert record["exit_result_name"] == "EXEC"


@yak_test()
async def test_passing_arguments(yak: Yak) -> None:
    async def f(args1: List[str], args2: List[str]) -> None:
        result = await yak.run("root//:echo_args", *args1, *args2)
        assert result.stdout.strip() == " ".join(args2)

    await f(["--"], ["val", "--long", "-s", "spa  ces"])
    await f(["--"], ["val", "--", "test"])
    await f([], ["val", "--", "x"])
    await expect_failure(
        yak.run("root//:echo_args", "--not-a-flag"),
        stderr_regex=r"unexpected argument '--not-a-flag'",
    )


@yak_test()
async def test_executable_fail_to_build(yak: Yak) -> None:
    await expect_failure(
        yak.run("root//:build_fail"),
        stderr_regex=r"Failed to build",
    )


# `run_args_without_separator` is a hard error, so yak fails a `run` whose
# arguments contain no `--`.
@yak_test()
async def test_run_args_without_separator(yak: Yak) -> None:
    await expect_failure(
        yak.run("root//:echo_args", "my_arg"),
        stderr_regex="`yak run` will require a `--` separator before target arguments",
    )
    await expect_failure(
        yak.run("root//:echo_args", "val", "--long"),
        stderr_regex="`yak run` will require a `--` separator before target arguments",
    )


@yak_test()
async def test_input(yak: Yak) -> None:
    await yak.run("root//:check_input_test", input=b"test")


@yak_test()
async def test_change_cwd(yak: Yak, tmp_path: Path) -> None:
    result = await yak.run(
        "root//:print_cwd",
        f"--chdir={tmp_path}",
    )
    assert tmp_path.resolve() == Path(result.stdout.strip()).resolve()


@yak_test()
async def test_dont_change_cwd(yak: Yak) -> None:
    result = await yak.run("root//:print_cwd")
    assert yak.cwd == Path(result.stdout.strip())
