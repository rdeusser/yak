# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import os
import shutil
import signal
import time
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env
from e2e_util.helper.golden import (
    golden,
    sanitize_daemon_stderr,
    sanitize_stacktrace,
    sanitize_stderr,
)
from e2e_util.helper.utils import is_running_on_windows, read_invocation_record


@yak_test(write_invocation_record=True)
async def test_action_error(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//:action_fail"),
        stderr_regex="Failed to build 'root//:action_fail",
    )
    error = res.invocation_record().single_error()
    assert error["category"] == "USER"
    # This test is unfortunately liable to break as a result of refactorings, since this is not
    # stable. Feel free to delete it if it becomes a problem.
    assert error["source_location"].startswith(
        "yak_build_api/src/actions/errors/action_error.rs::ActionError::"
    )


@yak_test(write_invocation_record=True)
async def test_missing_outputs(yak: Yak) -> None:
    # FIXME(JakobDegen): This doesn't work with non-local-only actions
    res = await expect_failure(
        yak.build("//:missing_outputs", "--local-only"),
        stderr_regex="Failed to build 'root//:missing_outputs",
    )
    error = res.invocation_record().single_error()
    assert error["category"] == "USER"


@yak_test(write_invocation_record=True)
async def test_bad_url(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//:bad_url"),
        stderr_regex="Failed to build 'root//:bad_url",
    )
    error = res.invocation_record().single_error()
    # Also liable to break as a result of refactorings, feel free to update
    # FIXME(minglunli): This is a regression from before, the commented line is better and we should fix this
    assert "yak_http/src/lib.rs" in error["source_location"]
    # assert error["source_location"] == "yak_http/src/lib.rs::HttpError::SendRequest"


@yak_test(write_invocation_record=True)
async def test_attr_coercion(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//attr_coercion:int_rule"),
        stderr_regex="evaluating build file: `root//attr_coercion:YAK.fixture",
    )
    error = res.invocation_record().single_error()
    assert "StarlarkError::RuntimeType::" in error["source_location"]
    assert "STARLARK_VALUE" in error["tags"]
    assert "STARLARK_RUNTIME_TYPE_ERROR" in error["tags"]
    assert error["category_key"].endswith("starlark_runtime_type_error=VALUE_UNPACK")


@yak_test(write_invocation_record=True)
async def test_yak_fail(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//yak_fail:foobar"),
        stderr_regex="evaluating build file: `root//yak_fail:YAK.fixture`",
    )
    error = res.invocation_record().single_error()
    # Just make sure that despite there being no context on the error, we still report the right
    # metadata
    assert error["source_location"].startswith(
        "yak_interpreter_for_build/src/interpreter/functions/internals.rs::YakFail::"
    )


@yak_test(write_invocation_record=True)
async def test_starlark_fail_error_categorization(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//starlark_fail:foobar"),
        stderr_regex="evaluating build file: `root//starlark_fail:YAK.fixture`",
    )
    error = res.invocation_record().single_error()
    assert "StarlarkError::Fail::" in error["source_location"]
    assert error["source_area"] == "YAK"
    assert error["category"] == "USER"


@yak_test(write_invocation_record=True)
async def test_starlark_parse_error_categorization(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//starlark_parse_error:starlark_parse_error"),
        stderr_regex=".*Parse error:.*",
    )
    error = res.invocation_record().single_error()
    assert "StarlarkError::Parser::" in error["source_location"]
    assert error["tags"] == ["STARLARK_PARSER"]
    assert error["source_area"] == "YAK"
    assert error["category"] == "USER"


@yak_test(write_invocation_record=True)
async def test_starlark_scope_error_categorization(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//starlark_scope_error:value_err"),
        stderr_regex="evaluating build file: .* not found",
    )
    error = res.invocation_record().single_error()
    assert "StarlarkError::Scope::" in error["source_location"]
    assert error["tags"] == ["STARLARK_SCOPE"]
    assert error["source_area"] == "YAK"
    assert error["category"] == "USER"


@yak_test(write_invocation_record=True)
async def test_targets_error_categorization(yak: Yak) -> None:
    res = await expect_failure(
        yak.targets("//starlark_fail:foobar"),
        stderr_regex="evaluating build file: `root//starlark_fail:YAK.fixture`",
    )
    error = res.invocation_record().single_error()
    assert error["tags"] == ["INPUT", "STARLARK_FAIL"]
    assert error["category"] == "USER"

    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="fixtures/test_targets_error_categorization.golden.txt",
    )


@yak_test(write_invocation_record=True)
async def test_daemon_crash(yak: Yak) -> None:
    await yak.build()

    res = await expect_failure(
        yak.debug("crash", "panic"),
    )
    error = res.invocation_record().single_error()
    if is_running_on_windows():
        assert "transport error" in error["message"]
    else:
        assert "stream closed because of a broken pipe" in error["message"]

    assert error["tags"][0:3] == [
        "CLIENT_GRPC",
        "CLIENT_GRPC_STREAM",
        "SERVER_PANICKED",
    ]
    assert error["tags"][4].startswith("crash")
    assert "yakd stderr:\n" in error["message"]
    assert "panicked at" in error["message"]

    assert error["best_tag"] == "SERVER_PANICKED"
    category_key = error["category_key"]
    assert category_key.startswith("SERVER_PANICKED")

    # TODO dump stack trace on windows
    if not is_running_on_windows():
        assert "crash(" in category_key

    if not is_running_on_windows():
        golden(
            output=sanitize_stacktrace(res.stderr),
            rel_path="fixtures/test_daemon_crash.golden.txt",
        )


@yak_test(write_invocation_record=True)
@env("YAKD_STARTUP_TIMEOUT", "0")
@env("YAKD_STARTUP_INIT_TIMEOUT", "0")
async def test_connection_timeout(yak: Yak) -> None:
    res = await expect_failure(yak.targets(":"))
    assert "timed out before establishing connection to yak daemon" in res.stderr

    record = res.invocation_record()

    assert record["command_end"] is None
    assert record["has_command_result"] is False
    assert record["has_end_of_stream"] is False
    assert record["daemon_connection_failure"] is True
    assert record["daemon_was_started"] is None

    assert record.single_error()["category_key"] == "CLIENT_STARTUP_TIMEOUT"

    if not is_running_on_windows():
        golden(
            output=sanitize_stderr(res.stderr),
            rel_path="fixtures/test_connection_timeout.golden.txt",
        )


@yak_test(write_invocation_record=True)
async def test_daemon_abort(yak: Yak) -> None:
    await yak.build()

    res = await expect_failure(yak.debug("crash", "abort"))
    error = res.invocation_record().single_error()
    # The client recognizes a SIGABRT from the signal handler message in the
    # daemon's stderr. The daemon installs no such handler, so the client
    # reports a lost connection.
    assert "yakd stderr is empty" in error["message"]
    assert error["category_key"] == "DAEMON_DISCONNECT"


async def wait_for_daemon_pid(yak: Yak) -> int:
    for _ in range(10):
        time.sleep(1)
        status = await yak.status()
        if status.stderr != "no yakd running":
            return json.loads(status.stdout)["process_info"]["pid"]
    raise Exception("Failed to find yakd pid")


def read_daemon_cgroup(pid: int) -> str:
    for line in Path(f"/proc/{pid}/cgroup").read_text().splitlines():
        if line.startswith("0::/"):
            return line.removeprefix("0::")
    raise AssertionError(f"Could not find cgroup v2 entry for daemon PID {pid}")


def fake_dmesg_env(tmp_path: Path, output: str) -> tuple[dict[str, str], Path]:
    dmesg_output = tmp_path / "dmesg-output"
    dmesg_output.write_text(output)
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    fake_dmesg = fake_bin / "dmesg"
    fake_dmesg.write_text(
        '#!/bin/sh\n: > "$YAK_TEST_DMESG_CALLED"\nexec /bin/cat "$YAK_TEST_DMESG_OUTPUT"\n'
    )
    fake_dmesg.chmod(0o755)
    dmesg_called = tmp_path / "dmesg-called"
    return (
        {
            "YAK_TEST_DMESG_CALLED": str(dmesg_called),
            "YAK_TEST_DMESG_OUTPUT": str(dmesg_output),
            "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
        },
        dmesg_called,
    )


def current_dmesg_timestamp(seconds_ago: int = 0) -> str:
    monotonic_ns = time.clock_gettime_ns(time.CLOCK_MONOTONIC) - (
        seconds_ago * 1_000_000_000
    )
    seconds, nanoseconds = divmod(monotonic_ns, 1_000_000_000)
    return f"{seconds}.{nanoseconds // 1_000:06d}"


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_pre_daemon_oom_record_does_not_mask_daemon_crash(
    yak: Yak, tmp_path: Path
) -> None:
    pre_daemon_timestamp = current_dmesg_timestamp(seconds_ago=60)
    await yak.build()
    daemon_cgroup = read_daemon_cgroup(await wait_for_daemon_pid(yak)).removeprefix(
        "/"
    )

    dmesg_env, dmesg_called = fake_dmesg_env(
        tmp_path,
        f"<6>[{pre_daemon_timestamp}] oomd kill: 81.28 80.05 42.78 {daemon_cgroup} 1000 ruleset:[test]\n",
    )

    res = await expect_failure(yak.debug("crash", "panic", env=dmesg_env))
    error = res.invocation_record().single_error()
    assert dmesg_called.exists()
    assert "DAEMON_OOM_KILLED" not in error["tags"]
    assert error["best_tag"] == "SERVER_PANICKED"


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_current_oomd_record_marks_daemon_crash_as_oom(
    yak: Yak, tmp_path: Path
) -> None:
    await yak.build()
    daemon_cgroup = read_daemon_cgroup(await wait_for_daemon_pid(yak)).removeprefix(
        "/"
    )
    dmesg_env, dmesg_called = fake_dmesg_env(
        tmp_path,
        f"<6>[{current_dmesg_timestamp()}] oomd kill: 81.28 80.05 42.78 {daemon_cgroup} 1000 ruleset:[test]\n",
    )

    res = await expect_failure(yak.debug("crash", "panic", env=dmesg_env))
    error = res.invocation_record().single_error()
    assert dmesg_called.exists()
    assert "DAEMON_OOM_KILLED" in error["tags"]
    assert error["best_tag"] == "DAEMON_OOM_KILLED"


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_oomd_kill_in_descendant_cgroup_does_not_mask_daemon_crash(
    yak: Yak, tmp_path: Path
) -> None:
    await yak.build()
    daemon_cgroup = read_daemon_cgroup(await wait_for_daemon_pid(yak)).removeprefix(
        "/"
    )
    dmesg_env, dmesg_called = fake_dmesg_env(
        tmp_path,
        f"<6>[{current_dmesg_timestamp()}] oomd kill: 81.28 80.05 42.78 {daemon_cgroup}/child 1000 ruleset:[test]\n",
    )

    res = await expect_failure(yak.debug("crash", "panic", env=dmesg_env))
    error = res.invocation_record().single_error()
    assert dmesg_called.exists()
    assert "DAEMON_OOM_KILLED" not in error["tags"]
    assert error["best_tag"] == "SERVER_PANICKED"


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_kernel_oom_victim_pid_marks_daemon_crash_as_oom(
    yak: Yak, tmp_path: Path
) -> None:
    await yak.build()
    daemon_pid = await wait_for_daemon_pid(yak)
    dmesg_env, dmesg_called = fake_dmesg_env(
        tmp_path,
        f"<6>[{current_dmesg_timestamp()}] Memory cgroup out of memory: Killed process {daemon_pid} (yak) total-vm:1000kB\n",
    )

    res = await expect_failure(yak.debug("crash", "panic", env=dmesg_env))
    error = res.invocation_record().single_error()
    assert dmesg_called.exists()
    assert "DAEMON_OOM_KILLED" in error["tags"]
    assert error["best_tag"] == "DAEMON_OOM_KILLED"


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_kernel_oom_for_other_pid_does_not_mask_daemon_crash(
    yak: Yak, tmp_path: Path
) -> None:
    await yak.build()
    other_pid = await wait_for_daemon_pid(yak) + 1
    dmesg_env, dmesg_called = fake_dmesg_env(
        tmp_path,
        f"<6>[{current_dmesg_timestamp()}] Memory cgroup out of memory: Killed process {other_pid} (yak) total-vm:1000kB\n",
    )

    res = await expect_failure(yak.debug("crash", "panic", env=dmesg_env))
    error = res.invocation_record().single_error()
    assert dmesg_called.exists()
    assert "DAEMON_OOM_KILLED" not in error["tags"]
    assert error["best_tag"] == "SERVER_PANICKED"


@yak_test(skip_for_os=["windows"])
async def test_daemon_killed(yak: Yak, tmp_path: Path) -> None:
    record = tmp_path / "record.json"
    build = await yak.build(
        ":slow_action", "--unstable-write-invocation-record", str(record)
    ).start()

    pid = await wait_for_daemon_pid(yak)
    os.kill(pid, signal.SIGKILL)
    await build.communicate()  # Wait for the client to exit

    error = read_invocation_record(record).single_error()
    assert error["category_key"] == "DAEMON_DISCONNECT"
    assert error["category"] == "ENVIRONMENT"


@yak_test(write_invocation_record=True)
async def test_build_file_race(yak: Yak) -> None:
    target = "//file_busy:file"
    # first build
    file_path = (await yak.build(target)).get_build_report().output_for_target(target)

    # Open the file for writing and keep it open
    f = open(file_path, "w")
    # build again, source code has changed, binary must be rebuilt
    build = yak.build(
        target,
        "--show-output",
        "-c",
        "test.cache_buster=2",
    )

    if is_running_on_windows():
        res = await expect_failure(build)

        error = res.invocation_record().single_error()
        assert error["best_tag"] == "IO_MATERIALIZER_FILE_BUSY"
        assert error["category"] == "ENVIRONMENT"
    else:
        await build

    f.close()


@pytest.mark.remote_execution
@yak_test(write_invocation_record=True)
async def test_download_failure(yak: Yak) -> None:
    # Upload action if necessary
    await yak.build("//:run_action", "--remote-only")
    await yak.clean()
    res = await expect_failure(
        yak.build(
            "//:run_action",
            env={"YAK_TEST_FAIL_RE_DOWNLOADS": "true"},
        )
    )
    error = res.invocation_record().single_error()
    assert error["category"] == "INFRA"
    assert error["category_key"] == "RE_NOT_FOUND:DIGEST_NOT_FOUND"
    assert (
        "Your build requires materializing an artifact that has expired in the RE CAS and yak does not have it. This likely happened because your yak daemon has been online for a long time. This error is currently unrecoverable. To proceed, you should restart yak using `yak killall`."
        in res.stderr
    )


@pytest.mark.remote_execution
@yak_test(write_invocation_record=True)
async def test_declared_artifact_download_failure(yak: Yak) -> None:
    res = await expect_failure(
        yak.build(
            "//:declared_file",
            env={"YAK_TEST_FAIL_RE_DOWNLOADS": "true"},
        )
    )
    error = res.invocation_record().single_error()
    assert error["best_tag"] == "DECLARED_ARTIFACT_NOT_FOUND"
    assert error["category"] == "USER"


@pytest.mark.remote_execution
@yak_test(write_invocation_record=True)
async def test_declared_tree_download_failure(yak: Yak) -> None:
    res = await expect_failure(
        yak.build(
            "//:declared_tree",
            env={"YAK_TEST_FAIL_RE_DOWNLOADS": "true"},
        )
    )
    error = res.invocation_record().single_error()
    assert error["best_tag"] == "DECLARED_ARTIFACT_NOT_FOUND"
    assert error["category"] == "USER"


@pytest.mark.remote_execution
@yak_test(write_invocation_record=True)
async def test_re_execute_failure(yak: Yak) -> None:
    # Upload action if necessary
    await yak.build("//:run_action", "--remote-only")
    await yak.clean()
    res = await expect_failure(
        yak.build(
            "//:run_action",
            "--no-remote-cache",
            env={"YAK_TEST_FAIL_RE_EXECUTE": "true"},
        )
    )
    error = res.invocation_record().single_error()
    assert error["category_key"] == "RE_FAILED_PRECONDITION:UNKNOWN"


@pytest.mark.remote_execution
@yak_test(write_invocation_record=True)
async def test_local_incompatible(yak: Yak) -> None:
    res = await expect_failure(
        yak.build(
            "//:local_run_action",
            "--remote-only",
            "--no-remote-cache",
        )
    )
    assert "Incompatible executor preferences" in res.stderr

    error = res.invocation_record().single_error()
    assert error["category"] == "USER"
    assert (
        error["category_key"] == "IncompatibleExecutorPreferences:ANY_ACTION_EXECUTION"
    )


@yak_test(write_invocation_record=True)
@env("YAK_TEST_INIT_DAEMON_ERROR", "true")
async def test_daemon_startup_error(yak: Yak) -> None:
    res = await expect_failure(yak.targets(":"))
    assert "Injected init daemon error" in res.stderr
    assert "Error initializing DaemonStateData" in res.stderr

    error = res.invocation_record().single_error()
    assert "DAEMON_CONNECT" in error["tags"]
    assert "DAEMON_STATE_INIT_FAILED" in error["tags"]

    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="fixtures/test_daemon_startup_error.golden.txt",
    )


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
@env("YAK_TEST_DAEMON_STARTUP_SIGNAL", "true")
async def test_daemon_startup_signal(yak: Yak) -> None:
    res = await expect_failure(yak.targets(":"))
    error = res.invocation_record().single_error()
    assert error["category_key"] == "DAEMON_STARTUP_FAILED:SIGTERM"

    golden(
        output=sanitize_daemon_stderr(res.stderr),
        rel_path="fixtures/test_daemon_startup_signal.golden.txt",
    )


@yak_test(write_invocation_record=True)
@env("YAK_TEST_FAIL_STREAMING", "true")
async def test_client_streaming_error(yak: Yak) -> None:
    res = await expect_failure(yak.targets(":"))
    assert "Injected client streaming error" in res.stderr

    error = res.invocation_record().single_error()
    assert "Injected client streaming error" in error["message"]

    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="fixtures/test_client_streaming_error.golden.txt",
    )


@yak_test(write_invocation_record=True)
async def test_action_error_has_categorization(yak: Yak) -> None:
    res = await expect_failure(
        yak.build("//fail_action:error_handler_produced_error_categories"),
        stderr_regex="Action sub-errors produced by error handlers",
    )

    error = res.invocation_record().single_error()
    assert "ACTION_COMMAND_FAILURE" in error["tags"]
    assert error["category_key"] == "ACTION_COMMAND_FAILURE:FirstError"


@yak_test(write_invocation_record=True, skip_for_os=["windows"])
@env("YAK_TEST_INIT_DATA_SLEEP_SECS", "120")
@env("YAKD_STARTUP_INIT_TIMEOUT", "5")
async def test_init_data_timeout(yak: Yak) -> None:
    res = await expect_failure(yak.targets(":"))
    record = res.invocation_record()
    error = record.single_error()

    assert error["category_key"] == "CLIENT_STARTUP_TIMEOUT"
    golden(
        output=sanitize_daemon_stderr(res.stderr),
        rel_path="fixtures/test_init_timeout.golden.txt",
    )


@yak_test(
    skip_for_os=["darwin", "windows"],
    write_invocation_record=True,
)
async def test_nix_errno(yak: Yak) -> None:
    await yak.build(":run_action", "--show-output")
    shutil.rmtree(yak.cwd / "yak-out/v2")

    res = await expect_failure(
        yak.targets(":"),
        stderr_regex="Failed to stat.*ENOENT: No such file or directory",
    )
    error = res.invocation_record().single_error()
    assert error["category_key"] == "NIX:ENOENT"
