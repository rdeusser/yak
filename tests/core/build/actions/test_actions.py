# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import hashlib
import json
import os
import platform
import socket
from pathlib import Path

from aiohttp import web
import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


@yak_test(data_dir="actions")
async def test_write_json(yak: Yak) -> None:
    result = await yak.build("//write_json:", "-c", "write_json.content=default")

    build_report = result.get_build_report()
    output = build_report.output_for_target("//write_json:absolute")
    for path in json.loads(output.read_text()):
        assert os.path.isabs(path), path

    # we need to test that with_inputs properly flows input dependencies through to consumers
    await yak.build("//write_json:with_inputs", "-c", "write_json.content=other")


@yak_test(data_dir="actions")
async def test_write_json_stack_overflow(yak: Yak) -> None:
    await expect_failure(
        yak.build("//write_json_stack_overflow:deep_write_json"),
        stderr_regex="stack overflow \\(internal error\\)",
    )
    await expect_failure(
        yak.build("//write_json_stack_overflow:deep_tset"),
        stderr_regex="stack overflow \\(internal error\\)",
    )


@yak_test(data_dir="actions")
async def test_copies_files(yak: Yak) -> None:
    result = await yak.build(
        "//copy:file_uses_declared_output",
        "//copy:file_uses_declared_output_as_output",
        "//copy:file_declares_output",
    )
    build_report = result.get_build_report()

    output = build_report.output_for_target("//copy:file_uses_declared_output")
    assert output.read_text().rstrip() == "some file"

    output = build_report.output_for_target(
        "//copy:file_uses_declared_output_as_output"
    )
    assert output.read_text().rstrip() == "some file"

    output = build_report.output_for_target("//copy:file_declares_output")
    assert output.read_text().rstrip() == "some file"

    await expect_failure(
        yak.build("//copy:fails_on_invalid_src"),
        stderr_regex="Type of parameter `src`",
    )

    await expect_failure(
        yak.build("//copy:fails_on_invalid_dest"),
        stderr_regex="Type of parameter `dest`",
    )


# In Windows, we convert all symlinks to be absolute and mostly canonical
def get_canonicalized_for_windows(dest: Path, relative_link: str) -> str:
    return "\\\\?\\" + os.path.realpath(dest.parent / relative_link)


@yak_test(data_dir="actions")
async def test_symlink_dir(yak: Yak) -> None:
    result = await yak.build("//symlinked_dir:")
    build_report = result.get_build_report()
    output = build_report.output_for_target("//symlinked_dir:out")

    dest1 = output / "dir1" / "dir1_1" / "file1.txt"
    dest2 = output / "dep.txt"
    dest3 = output / "subdir" / "dir1" / "dir1_1" / "file1.txt.suffix"
    dest4 = output / "subdir" / "dep.txt.suffix"
    dep_output = build_report.output_for_target("//symlinked_dir:dep").resolve()

    # Example subdir: yak-out/v2/art/root/a59b783ba97fcd85891ddb2e62fbfebb/symlinked_dir/__out__/out/dir1/dir1_1
    expected_link1 = "../" * 10 + "symlinked_dir/dir1/dir1_1/file1.txt"
    expected_link2 = os.path.relpath(dep_output, dest2.parent)
    expected_link3 = "../" * 11 + "symlinked_dir/dir1/dir1_1/file1.txt"
    expected_link4 = os.path.relpath(dep_output, dest4.parent)

    if platform.system() == "Windows":
        expected_link1 = get_canonicalized_for_windows(dest1, expected_link1)
        expected_link2 = get_canonicalized_for_windows(dest2, expected_link2)
        expected_link3 = get_canonicalized_for_windows(dest3, expected_link3)
        expected_link4 = get_canonicalized_for_windows(dest4, expected_link4)

    assert dest1.is_symlink()
    assert dest2.is_symlink()
    assert dest3.is_symlink()
    assert dest4.is_symlink()

    assert os.readlink(dest1) == expected_link1
    assert os.readlink(dest2) == expected_link2
    assert os.readlink(dest3) == expected_link3
    assert os.readlink(dest4) == expected_link4

    assert dest1.read_text().strip() == "dir1_1 out contents"
    assert dest2.read_text().strip() == "dep contents"
    assert dest3.read_text().strip() == "dir1_1 out contents"
    assert dest4.read_text().strip() == "dep contents"


@yak_test(
    data_dir="actions",
    # See note on test_symlink_dir
)
async def test_symlink_dir_associated_artifacts(yak: Yak) -> None:
    result = await yak.build("//symlinked_dir:symlinked_transitive_files_target")
    build_report = result.get_build_report()
    output = build_report.output_for_target(
        "//symlinked_dir:symlinked_transitive_files_target"
    )

    # This is set up in symlinked_dir:target_with_tdep
    dest = output / "out_file"

    # The direct src of the symlink is handled properly
    assert dest.is_symlink()

    # The transitive dependency of the symlink is not handled properly
    assert not (output / "tdep1").exists()


@yak_test(data_dir="actions")
async def test_simple_run(yak: Yak) -> None:
    result = await yak.build("//run:runs_simple_script")
    output = result.get_build_report().output_for_target("//run:runs_simple_script")
    if platform.system() == "Windows":
        assert output.read_text() == "foo\nrun\\src.txt\nbar\n"
    else:
        assert output.read_text() == "foo\nrun/src.txt\nbar\n"

    result = await yak.build("//run:runs_simple_script_as_exe")
    output = result.get_build_report().output_for_target(
        "//run:runs_simple_script_as_exe"
    )
    if platform.system() == "Windows":
        assert output.read_text() == "foo\nrun\\src.txt\nbar\n"
    else:
        assert output.read_text() == "foo\nrun/src.txt\nbar\n"

    result = await yak.build("//run:runs_script_locally")
    output = result.get_build_report().output_for_target("//run:runs_script_locally")
    assert output.read_text().strip() == socket.gethostname()

    result = await yak.build("//run:runs_script_locally_outputs_symlink")
    output = result.get_build_report().output_for_target(
        "//run:runs_script_locally_outputs_symlink"
    )
    assert output.is_symlink()

    await expect_failure(
        yak.build("//run:rejects_zero_outputs"),
        stderr_regex="expected at least one output artifact",
    )

    await expect_failure(
        yak.build("//run:rejects_bad_args"),
        stderr_regex="Type of parameter `arguments` doesn't match",
    )


@yak_test(data_dir="actions")
async def test_anon_targets(yak: Yak) -> None:
    await yak.build("//anon:")

    await expect_failure(
        yak.build("//anon_invalid_defaults/source:default_source_fails"),
        stderr_regex="Anon targets do not support default values for `attrs.source\\(\\)`, specify `source_attr` explicitly",
    )

    await expect_failure(
        yak.build("//anon_invalid_defaults/dep:default_dep_fails"),
        stderr_regex="Anon targets do not support default values for `attrs.dep\\(\\)`, specify `dep_attr` explicitly",
    )

    await expect_failure(
        yak.build("//anon_invalid_defaults/arg:default_arg_fails"),
        stderr_regex="Anon targets do not support default values for `attrs.arg\\(\\)`, specify `arg_attr` explicitly",
    )

    await expect_failure(
        yak.build("//anon_invalid_defaults/arg:arg_not_compatible"),
        stderr_regex="Arg attribute must have `anon_target_compatible` set to `True`",
    )

    await expect_failure(
        yak.build("//anon_invalid_defaults/promise_artifact:bad_short_path"),
        stderr_regex="assert_short_path\\(\\) was called with `short_path = WRONG_PATH`",
    )

    await expect_failure(
        yak.build("//anon_invalid_defaults/anon_rule:bad_anon_rule"),
        stderr_regex="Attr type `attrs.plugin_dep\\(\\)` is not supported for anon rules",
    )


@yak_test(data_dir="actions")
async def test_download_file(yak: Yak) -> None:
    routes = web.RouteTableDef()

    attempt = 0
    body: bytes = b"foobar"
    sha1 = hashlib.sha1(body).hexdigest()

    @routes.get("/")
    async def hello(request: web.Request) -> web.Response:
        nonlocal attempt
        attempt += 1
        if attempt > 2:
            return web.Response(body=body)
        if attempt > 1:
            return web.Response(status=500)
        return web.Response(status=429)

    app = web.Application()
    app.add_routes(routes)

    sock = socket.socket()
    sock.bind(("localhost", 0))

    runner = web.AppRunner(app)
    await runner.setup()
    site = web.SockSite(runner, sock)
    await site.start()

    port = sock.getsockname()[1]
    url = f"http://localhost:{port}"
    await yak.build(
        "//download_file:", "-c", f"test.sha1={sha1}", "-c", f"test.url={url}"
    )

    await runner.cleanup()

    # The download has only a SHA-1 checksum, which the default SHA-256 digest
    # configuration cannot defer, so yak downloads the file at once. The
    # server sees the two failed requests that yak retries and the request
    # that succeeds.
    assert attempt == 3


@yak_test(data_dir="actions")
async def test_download_file_timeout_after_retries(yak: Yak) -> None:
    routes = web.RouteTableDef()

    body: bytes = b"foobar"
    sha1 = hashlib.sha1(body).hexdigest()

    @routes.get("/always_times_out")
    async def always_times_out(request: web.Request) -> web.Response:
        await asyncio.sleep(3)
        return web.Response(body=body)

    attempt = 0

    @routes.get("/times_out_twice")
    async def times_out_twice(request: web.Request) -> web.Response:
        nonlocal attempt
        attempt += 1
        if attempt > 2:
            return web.Response(body=body)
        await asyncio.sleep(3)
        return web.Response(body=body)

    app = web.Application()
    app.add_routes(routes)

    sock = socket.socket()
    sock.bind(("localhost", 0))

    runner = web.AppRunner(app)
    await runner.setup()
    site = web.SockSite(runner, sock)
    await site.start()

    port = sock.getsockname()[1]
    url = f"http://localhost:{port}"

    # These are daemon startup configs, need these to be written in a yakconfig rather
    # than passed as an invocation config.
    #
    # Add an aggressive read timeout.
    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[http]\nread_timeout_ms = 50\n")

    await expect_failure(
        yak.build(
            "//download_file:",
            "-c",
            f"test.sha1={sha1}",
            "-c",
            f"test.url={url}/always_times_out",
        ),
        stderr_regex="Timed out while making request to",
    )

    result = await yak.build(
        "//download_file:",
        "-c",
        f"test.sha1={sha1}",
        "-c",
        f"test.url={url}/times_out_twice",
    )
    assert "Retrying a HTTP error after" in result.stderr

    await runner.cleanup()


@yak_test(data_dir="actions")
async def test_invalid_command(yak: Yak) -> None:
    await expect_failure(
        yak.build("//run_bad:run_invalid_command_local"),
        stderr_regex="non-zero exit code.*no exit code",
    )


@pytest.mark.remote_execution
@yak_test(data_dir="actions")
async def test_invalid_command_remote(yak: Yak) -> None:
    if platform.system() == "Linux":
        expected_error = "non-zero exit code"
    else:
        expected_error = "cannot find binary path"
    await expect_failure(
        yak.build("//run_bad:run_invalid_command_remote"),
        stderr_regex=expected_error,
    )


@yak_test(data_dir="actions")
async def test_exit_code(yak: Yak) -> None:
    await expect_failure(
        yak.build("//run_bad:run_odd_exit_code"),
        stderr_regex="non-zero exit code 45",
    )
    # Linux does not allow negative exit codes
    if platform.system() == "Windows":
        await expect_failure(
            yak.build("//run_bad:run_negative_exit_code"),
            stderr_regex="non-zero exit code -65",
        )


@yak_test(data_dir="actions")
async def test_artifact_cycle(yak: Yak) -> None:
    await expect_failure(
        yak.build("//run_invalid:artifact_cycle"),
        stderr_regex="Recursion limit exceeded",
    )


@yak_test(data_dir="actions")
async def test_associated_artifacts(yak: Yak) -> None:
    await yak.build("//associated_artifacts:check_artifacts")


@yak_test(data_dir="actions")
async def test_associated_artifacts_transitive_dep(yak: Yak) -> None:
    await yak.build("//associated_artifacts:check_dropped_artifacts")


@yak_test(data_dir="actions")
async def test_failure_has_wall_time(yak: Yak) -> None:
    await expect_failure(
        yak.build("//run_bad:run_odd_exit_code"),
        stderr_regex="non-zero exit code 45",
    )

    wall_time = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "wall_time_us",
    )

    assert wall_time
    print(wall_time)
    print(
        await filter_events(yak, "Event", "data", "SpanEnd", "data", "ActionExecution")
    )
    for time in wall_time:
        assert time > 0


@yak_test(data_dir="actions")
async def test_local_action_has_input_size(yak: Yak) -> None:
    await yak.build("//run:runs_script_locally")
    input_size = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "input_files_bytes",
    )

    assert input_size

    if platform.system() == "Windows":
        assert input_size[0] == 448
    else:
        assert input_size[0] == 416


@yak_test(data_dir="actions")
async def test_remote_action_has_input_size(yak: Yak) -> None:
    await yak.build("//run:runs_simple_script_remote")
    input_size = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "input_files_bytes",
    )

    assert input_size

    if platform.system() == "Windows":
        assert input_size[0] == 448
    else:
        assert input_size[0] == 416


@yak_test(data_dir="actions")
async def test_action_invalidation_tracking(yak: Yak) -> None:
    with open(yak.cwd / ".yakconfig", "a") as yakconfig:
        yakconfig.write("[yak]\n")
        yakconfig.write("invalidation_tracking_enabled = true\n")
        yakconfig.write("[yak]\n")
        yakconfig.write("invalidation_tracking_enabled = true\n")

    await yak.build("//run:runs_simple_script")
    invalidation_info = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "invalidation_info",
    )

    assert invalidation_info
    assert invalidation_info[0]["changed_file"] is None

    with open(yak.cwd / "run" / "src.txt", "a") as srcfile:
        srcfile.write("more data\n")

    await yak.build("//run:runs_simple_script")
    invalidation_info = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "invalidation_info",
    )

    assert invalidation_info
    assert invalidation_info[0]["changed_file"] == {}


@yak_test(data_dir="actions")
async def test_target_rule_type_name(yak: Yak) -> None:
    await yak.build("//run:runs_simple_script", "//copy:file_uses_declared_output")

    target_rule_type_name = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "target_rule_type_name",
    )

    assert len(target_rule_type_name) == 2
    assert "copy_file" in target_rule_type_name
    assert "run_command" in target_rule_type_name

    await expect_failure(
        yak.build("//run_bad:run_odd_exit_code"),
        stderr_regex="non-zero exit code 45",
    )

    target_rule_type_name = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "target_rule_type_name",
    )

    assert len(target_rule_type_name) == 1
    assert "run_odd_exit_code" in target_rule_type_name


@yak_test(data_dir="actions")
async def test_anon_target_rule_type_name(yak: Yak) -> None:
    await yak.build("//anon:build")

    actions = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
    )
    anon_actions = [
        action for action in actions if "AnonTarget" in action["key"]["owner"]
    ]

    assert anon_actions, actions
    assert {action["target_rule_type_name"] for action in anon_actions} == {"_builder"}
