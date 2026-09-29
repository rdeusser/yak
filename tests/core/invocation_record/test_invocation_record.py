# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import json
import platform
import signal
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden, sanitize_stderr
from e2e_util.helper.utils import read_invocation_record

# FIXME(JakobDegen): Flakey in CI
if False:

    @yak_test(skip_for_os=["windows"])
    async def test_has_end_of_stream_false(yak: Yak, tmp_path: Path) -> None:
        hang_path = tmp_path / "hang_path"
        record = tmp_path / "record.json"

        cmd = await yak.build(
            ":hang",
            "-c",
            f"test.hang_path={hang_path}",
            "--unstable-write-invocation-record",
            str(record),
            "--local-only",
            "--no-remote-cache",
        ).start()

        for _ in range(10):
            if hang_path.exists():
                break
            await asyncio.sleep(1)
        else:
            print(await cmd.communicate())
            raise Exception(f"Signal file never created: {hang_path}")

        cmd.send_signal(signal.SIGINT)
        await cmd.communicate()

        record = read_invocation_record(record)

        assert not record["has_end_of_stream"]
        assert not record["has_command_result"]


@yak_test(write_invocation_record=True)
async def test_has_end_of_stream_true(yak: Yak) -> None:
    res = await yak.build(":pass")

    record = res.invocation_record()

    assert record["has_end_of_stream"]
    assert record["has_command_result"]
    assert record["repo_path"] == str(yak.cwd)


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_has_no_command_result(yak: Yak) -> None:
    # Start the daemon
    await yak.build()

    status = json.loads((await yak.status()).stdout)
    pid = status["process_info"]["pid"]

    result = await expect_failure(
        yak.build(
            ":kill",
            "-c",
            f"test.pid={pid}",
            "--local-only",
            "--no-remote-cache",
        ),
        stderr_regex="yak daemon event bus encountered an error",
    )

    record = result.invocation_record()

    assert record["has_end_of_stream"]
    assert not record["has_command_result"]

    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="fixtures/test_has_no_command_result.golden.txt",
    )


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_metadata(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build()

    record = res.invocation_record()

    metadata = record["metadata"]["strings"]
    assert metadata["os"] == platform.system().lower()
    assert "username" not in metadata


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_client_metadata(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build(
        "--client-metadata=foo=bar",
        "--client-metadata=id=baz",
    )

    record = res.invocation_record()

    assert record["client_metadata"] == [
        {"key": "foo", "value": "bar"},
        {"key": "id", "value": "baz"},
    ]

    assert record["metadata"]["strings"]["client"] == "baz"


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_client_metadata_env(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build(
        "--client-metadata=foo=bar",
        "--client-metadata=id=baz",
        env={"YAK_CLIENT_METADATA": "env_foo=env_bar,id=foobar"},
    )

    record = res.invocation_record()

    assert record["client_metadata"] == [
        {"key": "env_foo", "value": "env_bar"},
        {"key": "id", "value": "foobar"},
        {"key": "foo", "value": "bar"},
        {"key": "id", "value": "baz"},
    ]

    assert record["metadata"]["strings"]["client"] == "baz"


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_client_metadata_clean(yak: Yak) -> None:
    # Start the daemon
    res = await yak.clean(
        "--client-metadata=foo=bar",
        "--client-metadata=id=baz",
    )

    record = res.invocation_record()

    assert record["client_metadata"] == [
        {"key": "foo", "value": "bar"},
        {"key": "id", "value": "baz"},
    ]

    assert record["metadata"]["strings"]["client"] == "baz"


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_client_metadata_debug(yak: Yak) -> None:
    # yak.debug() doesn't start the daemon, so we need to start it with a build
    await yak.build()

    res = await yak.debug(
        "hydration",
        "status",
        "--client-metadata=foo=bar",
        "--client-metadata=id=baz",
    )

    record = res.invocation_record()

    assert record["client_metadata"] == [
        {"key": "foo", "value": "bar"},
        {"key": "id", "value": "baz"},
    ]

    assert record["metadata"]["strings"]["client"] == "baz"


@yak_test(write_invocation_record=True)
async def test_action_error_message_in_record(yak: Yak) -> None:
    res = await expect_failure(yak.build(":fail"))

    record = res.invocation_record()

    assert len(record["errors"]) == 1
    assert (
        record["errors"][0]["message"]
        == "Failed to build 'root//:fail (<unspecified>)'"
    )
    assert "Hi from stderr!" in record["errors"][0]["telemetry_message"]


@yak_test(write_invocation_record=True)
async def test_non_action_error_message_in_record(yak: Yak) -> None:
    res = await expect_failure(yak.build(":missing_target"))

    record = res.invocation_record()

    assert len(record["errors"]) == 1
    assert record["errors"][0]["message"].startswith(
        "Unknown target `missing_target` from package `root//`"
    )


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_rule_type_names_ci(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build(
        ":duplicate",
        ":and_a_two",
        ":last_three",
        ":a_one",
        env={"CI": "true"},
    )

    record = res.invocation_record()

    assert record["target_rule_type_names"] == [
        "one",
        "pass_",
        "two",
    ]


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_rule_type_names_user(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build(
        ":and_a_two",
        ":last_three",
        ":a_one",
    )

    record = res.invocation_record()

    assert record["target_rule_type_names"] == [
        "one",
        "pass_",
        "two",
    ]


@yak_test(skip_for_os=["windows"], write_invocation_record=True)
async def test_rule_type_names_on_failure(yak: Yak) -> None:
    # Start the daemon
    res = await expect_failure(
        yak.build(
            ":fail",
            ":last_three",
            ":a_one",
        )
    )

    record = res.invocation_record()

    assert record["target_rule_type_names"] == [
        "fail",
        "one",
        "pass_",
    ]


@yak_test(write_invocation_record=True)
async def test_active_networks_kinds(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build()

    record = res.invocation_record()

    assert "active_networks_kinds" in record


@yak_test(write_invocation_record=True)
async def test_peak_memory_and_disk(yak: Yak) -> None:
    # Start the daemon
    res = await yak.build()

    record = res.invocation_record()

    assert (
        "peak_used_disk_space_bytes" in record and "peak_process_memory_bytes" in record
    )


@yak_test(write_invocation_record=True)
async def test_peak_stats(yak: Yak) -> None:
    res = await yak.build(
        ":run",
        "--no-remote-cache",
        "--local-only",
    )

    record = res.invocation_record()
    assert record
    assert record["max_in_progress_actions"] == 1
    assert record["max_in_progress_local_actions"] == 1
    assert record["max_in_progress_remote_actions"] == 0
    assert record["max_in_progress_remote_uploads"] == 0


@yak_test(write_invocation_record=True)
async def test_parallelism_logging(yak: Yak) -> None:
    # Test multiple parallelism values
    parallelism_values = [1, 4, 8]

    for parallelism in parallelism_values:
        # Test with different -j values to control concurrency
        res = await yak.build(
            ":pass",
            "-j",
            str(parallelism),
        )

        record = res.invocation_record()

        # Verify that command_options is present and contains parallelism data
        assert "command_options" in record
        command_options = record["command_options"]

        assert "configured_parallelism" in command_options
        assert "available_parallelism" in command_options

        # The configured parallelism should match what we passed via -j
        assert command_options["configured_parallelism"] == parallelism

        # The available parallelism should be a positive integer (system dependent)
        assert isinstance(command_options["available_parallelism"], int)
        assert command_options["available_parallelism"] > 0

    # Test without -j flag - configured_parallelism should be null
    res_no_j = await yak.build(
        ":pass",
    )

    record_no_j = res_no_j.invocation_record()

    # Verify that command_options is present and contains parallelism data
    assert "command_options" in record_no_j
    command_options_no_j = record_no_j["command_options"]

    assert "configured_parallelism" in command_options_no_j
    assert "available_parallelism" in command_options_no_j

    # When no -j is specified, configured_parallelism should equal available_parallelism
    assert (
        command_options_no_j["configured_parallelism"]
        == command_options_no_j["available_parallelism"]
    )

    # The available parallelism should still be a positive integer (system dependent)
    assert isinstance(command_options_no_j["available_parallelism"], int)
    assert command_options_no_j["available_parallelism"] > 0
