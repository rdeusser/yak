# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import hashlib
import json
import shlex
import typing
from pathlib import Path
from typing import Any

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException, BuildResult
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test, env
from e2e_util.helper.golden import golden, sanitize_stderr
from e2e_util.helper.utils import (
    expect_exec_count,
    filter_events,
    get_last_execution_kind,
    random_string,
    read_what_ran,
)

# Taken from data.proto
ACTION_EXECUTION_KIND_LOCAL = 1
ACTION_EXECUTION_KIND_ACTION_CACHE = 3
ACTION_EXECUTION_KIND_SIMPLE = 4
ACTION_EXECUTION_KIND_LOCAL_DEP_FILE = 7
ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE = 9
ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE = 10

CACHE_UPLOAD_REASON_LOCAL_EXECUTION = 0
CACHE_UPLOAD_REASON_DEP_FILE = 1


async def expect_only_dep_file_hit(yak: Yak) -> None:
    what_ran = await read_what_ran(yak)
    assert (
        len([x for x in what_ran if x["reproducer"]["executor"] == "LocalDepFileCache"])
        == 1
    )
    assert len(what_ran) == 1


async def check_execution_kind(
    yak: Yak,
    expecteds: list[int],
    ignored: typing.Optional[list[int]] = None,
) -> None:
    ignored = ignored or []
    execution_kinds = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )
    execution_kinds = [kind for kind in execution_kinds if kind not in ignored]
    assert len(execution_kinds) == len(expecteds)
    for actual, expected in zip(execution_kinds, expecteds):
        assert actual == expected


class MatchDepFilesEvent(typing.NamedTuple):
    remote_cache: bool
    checking_filtered_inputs: bool


async def check_match_dep_files_events(
    yak: Yak,
    expected_events: list[MatchDepFilesEvent],
) -> None:
    match_dep_files = await filter_events(
        yak, "Event", "data", "SpanStart", "data", "MatchDepFiles"
    )
    assert len(match_dep_files) == len(expected_events)

    for match, expected_event in zip(match_dep_files, expected_events):
        assert bool(match["remote_cache"]) == expected_event.remote_cache
        assert (
            bool(match["checking_filtered_inputs"])
            == expected_event.checking_filtered_inputs
        )


async def _get_execution_kind(yak: Yak) -> int:
    execution_kinds = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )
    return execution_kinds[0]


def touch(yak: Yak, name: str) -> None:
    """
    Append a random string to the marker in the file
    """
    with open(yak.cwd / name, encoding="utf-8") as f:
        text = f.read()

    with open(yak.cwd / name, "w", encoding="utf-8") as f:
        f.write(text.replace("__MARKER__", f"__MARKER__{random_string()}"))


async def _test_dep_files_impl(yak: Yak, use_content_based_paths: bool) -> None:
    """Common implementation for dep files tests."""
    # We query cache before we query dep file. Disable remote cache to make
    # sure that for the last build what-ran doesn't return cached entry.
    args = [
        "app:app",
        "--no-remote-cache",
        "-c",
        f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
    ]
    await yak.build(*args)
    await expect_exec_count(yak, 1)

    touch(yak, "app/app.h")
    await yak.build(*args)
    await expect_exec_count(yak, 1)

    touch(yak, "app/app.c")
    await yak.build(*args)
    await expect_exec_count(yak, 1)

    # //app:app doesn't use other.h and
    # using dep file this should build nothing.
    touch(yak, "app/other.h")
    await yak.build(*args)
    await expect_only_dep_file_hit(yak)
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        # A symlinked_dir command was re-run because app/other.h was changed
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    # Changing the command line itself should cause a rebuild.
    touch(yak, "app/other.h")
    await yak.build(*args, "-c", f"test.unused_command_line_param={random_string()}")
    await expect_exec_count(yak, 1)


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_with_content_based_paths(yak: Yak) -> None:
    await _test_dep_files_impl(yak, use_content_based_paths=True)


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_without_content_based_paths(yak: Yak) -> None:
    await _test_dep_files_impl(yak, use_content_based_paths=False)


async def _test_dep_files_in_same_package_impl(
    yak: Yak, use_content_based_paths: bool
) -> None:
    def make_args(
        used_input1_contents: str,
        used_input2_contents: str,
        unused_input1_contents: str,
        unused_input2_contents: str,
    ) -> list[str]:
        return [
            "app:simple_dep_file",
            "--no-remote-cache",
            "-c",
            f"test.used_input1_contents={used_input1_contents}",
            "-c",
            f"test.used_input2_contents={used_input2_contents}",
            "-c",
            f"test.unused_input1_contents={unused_input1_contents}",
            "-c",
            f"test.unused_input2_contents={unused_input2_contents}",
            "-c",
            f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
        ]

    used_input1_contents = random_string()
    used_input2_contents = random_string()
    unused_input1_contents = random_string()
    unused_input2_contents = random_string()

    await yak.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(yak, 1)

    used_input1_contents = random_string()
    await yak.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(yak, 1)

    used_input2_contents = random_string()
    await yak.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(yak, 1)

    unused_input1_contents = random_string()
    await yak.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_only_dep_file_hit(yak)
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    unused_input2_contents = random_string()
    await yak.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_only_dep_file_hit(yak)
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_package_with_content_based(yak: Yak) -> None:
    await _test_dep_files_in_same_package_impl(yak, use_content_based_paths=True)


@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_package_without_content_based(yak: Yak) -> None:
    await _test_dep_files_in_same_package_impl(yak, use_content_based_paths=False)


async def _test_dep_files_in_same_dir_impl(
    yak: Yak, use_content_based_paths: bool
) -> None:
    def make_args(
        used_input_contents: str,
        unused_input_contents: str,
    ) -> list[str]:
        return [
            "app:shared_dir_dep_file",
            "--no-remote-cache",
            "-c",
            f"test.used_input_contents={used_input_contents}",
            "-c",
            f"test.unused_input_contents={unused_input_contents}",
            "-c",
            f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
        ]

    used_input_contents = random_string()
    unused_input_contents = random_string()

    await yak.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_exec_count(yak, 1)

    used_input_contents = random_string()
    await yak.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_exec_count(yak, 1)

    unused_input_contents = random_string()
    await yak.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_only_dep_file_hit(yak)

    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_dir_with_content_based(yak: Yak) -> None:
    await _test_dep_files_in_same_dir_impl(yak, use_content_based_paths=True)


@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_dir_without_content_based(yak: Yak) -> None:
    await _test_dep_files_in_same_dir_impl(yak, use_content_based_paths=False)


async def get_cache_queries(yak: Yak) -> list[dict[str, Any]]:
    return await filter_events(
        yak,
        "Event",
        "data",
        "SpanStart",
        "data",
        "ExecutorStage",
        "stage",
        "CacheQuery",
    )


async def check_no_cache_query(yak: Yak) -> None:
    cache_queries = await get_cache_queries(yak)
    assert len(cache_queries) == 0


async def check_cache_query(yak: Yak) -> None:
    cache_queries = await get_cache_queries(yak)
    assert len(cache_queries) == 1


# Skipping on windows due to gcc dependency
@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_identical_action(yak: Yak) -> None:
    # For actions that have dep files, yak will query the local dep file cache to see
    # if an identical action is stored there. Otherwise, it will fall back to an action cache
    # look up (if enabled) and then to the full dep file query.
    # This test builds a target to build up a dep file cache, then builds the target again
    # with a no-op configuration change so that we hit the initial dep file lookup hit case.
    dummy1 = "dummy1"
    await yak.build(
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",  # Turn off remote cache query so we execute locally
        "-c",
        f"test.dummy_config={dummy1}",
    )
    await check_execution_kind(
        yak, [ACTION_EXECUTION_KIND_LOCAL], ignored=[ACTION_EXECUTION_KIND_SIMPLE]
    )

    dummy2 = "dummy2"
    await yak.build(
        "app:app_with_dummy_config",
        "--local-only",
        "-c",
        f"test.dummy_config={dummy2}",
    )
    # The result should be served by the local dep file cache BEFORE an action cache lookup
    await check_no_cache_query(yak)
    # Ignoring any simple actions because there can be either one or two symlink dir actions,
    # with the same dice key,
    # Not sure why but this feels like a DICE bug triggered by the yakconfig change.
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )
    # The MatchDepFilesStart span should indicate we only checked the depfile cache once
    await check_match_dep_files_events(
        yak, [MatchDepFilesEvent(remote_cache=False, checking_filtered_inputs=False)]
    )


async def _execution_kinds(yak: Yak) -> list[int]:
    return await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )


# The persisted local dep-file cache reloads across daemon restarts. After a restart the in-memory
# cache is gone, but the entry is reloaded from the sqlite db and, because the outputs are still
# materialized on disk, the identical action is served from the LOCAL_ACTION_CACHE without
# re-executing. The feature is on by default. The `_disabled` control proves this only happens with
# the feature enabled.
@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_persisted_across_restart(yak: Yak) -> None:
    args = [
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=dummy1",
    ]
    # First build populates both yak-out and the persisted dep-file cache.
    await yak.build(*args)
    # Killing the daemon drops the in-memory dep-file cache; the sqlite db and outputs persist.
    await yak.kill()
    # The rebuild reloads the entry from disk and serves the identical action from the local cache.
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE in kinds, kinds
    # Served by the reloaded local dep-file cache before any action-cache lookup.
    await check_no_cache_query(yak)


async def _prepare_persisted_dep_file_input_after_clean(
    yak: Yak,
) -> tuple[str, list[str]]:
    target = "root//app:consume_persisted_dep_file_output"
    args = [
        target,
        "--local-only",
        "--no-remote-cache",
    ]
    first = await yak.build(*args)
    output = first.get_build_report().output_for_target(target)
    assert output.read_text() == "output"

    await yak.kill()
    # Reload both actions from the persisted cache without final-output materialization. The
    # producer is accepted by `declare_match` and must be protected from stale cleanup.
    await yak.build(*args, "--materializations=none")
    kinds = await _execution_kinds(yak)
    assert kinds.count(ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE) == 2, kinds

    entries = (await yak.audit("deferred-materializer", "list")).stdout.splitlines()
    args_entries = [
        entry
        for entry in entries
        if entry.split("\t", 1)[0].endswith("/__persisted_dep_file_output_args__/args")
    ]
    assert len(args_entries) == 1, args_entries
    args_file = yak.cwd / args_entries[0].split("\t", 1)[0]
    producer_output = yak.cwd / args_file.read_text().strip()
    assert "/output_artifacts/" not in producer_output.as_posix(), producer_output
    assert producer_output.exists(), producer_output

    clean = await yak.clean("--stale=0s")
    assert producer_output.exists(), clean.stderr
    # Invalidate only the consumer. Its retained args file still names the producer output.
    touch(yak, "app/other.h")

    return target, args


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={
        "yak": {
            "defer_write_actions": "true",
            "restarter": "false",
            "sqlite_dep_file_state": "true",
        }
    },
)
async def test_persisted_dep_file_hit_survives_clean_stale(
    yak: Yak,
) -> None:
    target, args = await _prepare_persisted_dep_file_input_after_clean(yak)

    result = await yak.build(*args)
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="dep_files/golden/persisted_dep_file_hit_survives_clean_stale.stderr",
    )
    assert result.get_build_report().output_for_target(target).read_text() == "output"


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    # The persisted dep-file cache is gated on a daemon-startup yakconfig (read once when the daemon
    # boots, like the materializer/incremental state dbs), so it must be set here rather than via `-c`.
    extra_yak_config={"yak": {"sqlite_dep_file_state": "false"}},
)
async def test_dep_file_not_persisted_across_restart_when_disabled(yak: Yak) -> None:
    # Control for `test_dep_file_hit_persisted_across_restart`: with the feature disabled, a
    # restart loses the cache and the identical action re-executes locally.
    args = [
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=dummy1",
    ]
    await yak.build(*args)
    await yak.kill()
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE not in kinds, kinds
    assert ACTION_EXECUTION_KIND_LOCAL in kinds, kinds


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={"yak": {"sqlite_dep_file_state": "true"}},
)
async def test_changed_action_is_not_served_from_persisted_cache(yak: Yak) -> None:
    # The risk the persisted cache carries is not missing a hit, it is serving a stale output. A
    # reloaded entry is only usable for an action that is genuinely identical, so changing an input
    # across the restart must re-execute and produce the new content.
    def args(used_input_contents: str) -> list[str]:
        return [
            "app:dir_output_dep_file",
            "--local-only",
            "--no-remote-cache",
            "-c",
            f"test.used_input_contents={used_input_contents}",
            "--show-output",
        ]

    await yak.build(*args("used1"))
    await yak.kill()

    result = await yak.build(*args("used2"))
    kinds = await _execution_kinds(yak)
    # The persisted entry exists for this action, but its digests no longer match.
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE not in kinds, kinds
    assert ACTION_EXECUTION_KIND_LOCAL in kinds, kinds

    # The output on disk must be the newly produced one, not the reloaded entry's.
    outputs = result.get_target_to_build_output()
    assert len(outputs) == 1, outputs
    out_dir = (yak.cwd / next(iter(outputs.values()))).resolve()
    # The action echoes its used input, so this distinguishes the new tree from a reloaded one.
    assert (out_dir / "f").read_text() == "used2"


# A directory output's tree is not serialized into the dep-file db; only its fingerprint is. Across a
# restart the tree is rehydrated from the materializer (which persists+reloads it) and verified
# against that fingerprint, so an action with a directory output still hits the LOCAL_ACTION_CACHE.
@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={"yak": {"sqlite_dep_file_state": "true"}},
)
async def test_dir_output_dep_file_hit_persisted_across_restart(yak: Yak) -> None:
    args = [
        "app:dir_output_dep_file",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.used_input_contents=used1",
    ]
    await yak.build(*args)
    await yak.kill()
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE in kinds, kinds
    await check_no_cache_query(yak)


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={"yak": {"sqlite_dep_file_state": "true"}},
)
async def test_dir_output_dep_file_hit_persisted_without_content_based_paths(
    yak: Yak,
) -> None:
    # As above, but with a configuration-based output path, so the reload resolves the directory
    # without a content hash.
    args = [
        "app:dir_output_dep_file",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.used_input_contents=used1",
        "-c",
        "test.use_content_based_paths=false",
    ]
    await yak.build(*args)
    await yak.kill()
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE in kinds, kinds


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={"yak": {"sqlite_dep_file_state": "true"}},
)
async def test_flush_dep_files_clears_persisted_cache(yak: Yak) -> None:
    # `flush-dep-files` clears the in-memory cache synchronously, so the persisted rows must be gone
    # by the time it returns as well -- a row that outlives it would be reloaded after a restart and
    # serve an entry the user explicitly invalidated.
    args = [
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=dummy1",
    ]
    await yak.build(*args)
    await yak.debug("flush-dep-files")
    # Only a restart can distinguish a cleared db from a still-populated one: without the kill the
    # empty in-memory cache would produce a miss either way.
    await yak.kill()
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE not in kinds, kinds
    assert ACTION_EXECUTION_KIND_LOCAL in kinds, kinds


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    # The persisted cache re-validates reloaded outputs against the materializer's own state db, so
    # it refuses to start without it. Requesting it here should warn and stay disabled, not fail.
    extra_yak_config={
        "yak": {"sqlite_dep_file_state": "true", "sqlite_materializer_state": "false"}
    },
)
async def test_dep_file_persistence_disabled_without_materializer_state(
    yak: Yak,
) -> None:
    args = [
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=dummy1",
    ]
    await yak.build(*args)
    await yak.kill()
    await yak.build(*args)
    kinds = await _execution_kinds(yak)
    assert ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE not in kinds, kinds
    assert ACTION_EXECUTION_KIND_LOCAL in kinds, kinds


# Skipping on windows: simple_dep_file's action uses symlinks, which aren't supported there.
@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_across_configurations(yak: Yak) -> None:
    # An action whose outputs are all content-based and whose inputs are all eligible for dedupe has
    # a configuration-independent input directory, command line and output paths.
    #
    # This builds an identical, dedupe-eligible action under two different
    # target configurations (platform_a and platform_b).
    result_a = await yak.build(
        "app:simple_dep_file",
        "--target-platforms",
        "root//platforms:platform_a",
        "--local-only",
        "--no-remote-cache",  # Turn off remote cache query so we execute locally
        "--show-output",
    )
    await check_execution_kind(
        yak, [ACTION_EXECUTION_KIND_LOCAL], ignored=[ACTION_EXECUTION_KIND_SIMPLE]
    )

    result_b = await yak.build(
        "app:simple_dep_file",
        "--target-platforms",
        "root//platforms:platform_b",
        "--local-only",
        "--show-output",
    )
    await check_no_cache_query(yak)
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )
    await check_match_dep_files_events(
        yak, [MatchDepFilesEvent(remote_cache=False, checking_filtered_inputs=False)]
    )

    def single_output(result: Any) -> str:
        outputs = result.get_target_to_build_output()
        assert len(outputs) == 1, outputs
        return next(iter(outputs.values()))

    async def platform_config_hash(platform: str) -> str:
        # cquery keys its json output by the configured target label, which ends in the
        # configuration hash, e.g.
        # "root//app:simple_dep_file (root//platforms:platform_a#5baf920a753b3a79)".
        out = (
            await yak.cquery(
                "app:simple_dep_file",
                "--target-platforms",
                platform,
                "--output-attribute=name",
            )
        ).stdout
        keys = list(json.loads(out).keys())
        assert len(keys) == 1, keys
        label = keys[0]
        assert "#" in label, f"no configuration hash in cquery key: {label}"
        return label.split("#", 1)[1].split(")", 1)[0]

    reported_a = single_output(result_a)
    reported_b = single_output(result_b)

    hash_a = await platform_config_hash("root//platforms:platform_a")
    hash_b = await platform_config_hash("root//platforms:platform_b")
    assert hash_a != hash_b, f"platforms should have distinct config hashes: {hash_a}"
    assert hash_a in reported_a, f"{hash_a} not in {reported_a}"
    assert hash_b in reported_b, f"{hash_b} not in {reported_b}"
    assert reported_a.replace(hash_a, hash_b) == reported_b, (
        f"outputs paths should only differ in config hashes {reported_a} vs {reported_b}"
    )

    # Each configuration-hash path is a symlink into the deduplicated, configuration-independent
    # content-based location, so both resolve to the exact same file.
    path_a = (yak.cwd / reported_a).resolve()
    path_b = (yak.cwd / reported_b).resolve()
    assert path_a == path_b, (
        f"content-based output should resolve to one file: {path_a} vs {path_b}"
    )
    # ...and the file the cache hit points at must exist with the correct content.
    assert path_b.exists(), (
        f"cross-config cache hit output is not materialized: {path_b}"
    )
    assert path_b.read_text() == "output"


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_no_cross_config_hit_without_content_based_paths(yak: Yak) -> None:
    # The mirror of `test_dep_file_hit_across_configurations`: with non-content-based paths the
    # action is not configuration-independent, so building it under a second configuration must
    # execute rather than reuse the first configuration's entry.
    #
    # This pins the precondition that `hit_outputs_if_present` relies on. It verifies the outputs of
    # the *live* configuration are materialized rather than the candidate's, and those resolve to
    # the same path on disk only when the outputs are content-based. A non-content-based action
    # never reaches that check cross-configuration today, because its output paths are part of the
    # command line digest and so differ per configuration -- this test is what keeps that true.
    for platform in ["platform_a", "platform_b"]:
        await yak.build(
            "app:simple_dep_file",
            "--target-platforms",
            f"root//platforms:{platform}",
            "-c",
            "test.use_content_based_paths=false",
            "--local-only",
            "--no-remote-cache",
            "--show-output",
        )
        await check_execution_kind(
            yak,
            [ACTION_EXECUTION_KIND_LOCAL],
            ignored=[ACTION_EXECUTION_KIND_SIMPLE],
        )


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_select_divergent_actions_do_not_thrash_across_configurations(
    yak: Yak,
) -> None:
    # `app:select_divergent`'s action command line differs per configuration via select(), so its two
    # configurations are genuinely different actions. The local action cache
    # keeps a per-configuration entry, so building one configuration must not evict
    # another's entry: each configuration still gets its own incremental local action cache hit.

    # Build under platform_a -> executes and caches platform_a's entry.
    await yak.build(
        "app:select_divergent",
        "--target-platforms",
        "root//platforms:platform_a",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=d1",
    )
    await check_execution_kind(
        yak, [ACTION_EXECUTION_KIND_LOCAL], ignored=[ACTION_EXECUTION_KIND_SIMPLE]
    )

    # Build under platform_b -> a different action (different command line). It must execute (no
    # cross-config hit, since the identity differs) and must not evict platform_a's entry.
    await yak.build(
        "app:select_divergent",
        "--target-platforms",
        "root//platforms:platform_b",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.dummy_config=d1",
    )
    await check_execution_kind(
        yak, [ACTION_EXECUTION_KIND_LOCAL], ignored=[ACTION_EXECUTION_KIND_SIMPLE]
    )

    # Rebuild under platform_a with a no-op config change to force a DICE recompute. platform_a's
    # entry survived the platform_b build, so this is served by the local action cache.
    await yak.build(
        "app:select_divergent",
        "--target-platforms",
        "root//platforms:platform_a",
        "--local-only",
        "-c",
        "test.dummy_config=d2",
    )
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Changing ActionKey (by registering additional actions before the dep-file action
# during analysis) does not cause a dep-file cache miss when the dep-file action itself is
# identical -- the dep-file/output comparison is configuration- and ActionKey-independent.
@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_with_action_key_change(yak: Yak) -> None:
    await yak.build(
        "app:dep_file_with_preceding_actions",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.num_preceding_actions=0",
    )
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    # Add a preceding action, shifting the dep-file action's ActionKey index.
    # The dep-file action itself (command, inputs, outputs) is identical.
    await yak.build(
        "app:dep_file_with_preceding_actions",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.num_preceding_actions=1",
    )
    await check_execution_kind(
        yak,
        [ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
# This test tombstones the hash of the dep file produced by this action.
# The tombstone applies only when the materializer downloads the dep file from
# the CAS, so the action has to run remotely.
@pytest.mark.remote_execution
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
@env(
    "YAK_TEST_TOMBSTONED_DIGESTS",
    "e537c6611d7e2ba1c9b71248f7a0ca506e5a0f9a:78",
)
async def test_dep_files_ignore_missing_digests(yak: Yak, tmp_path: Path) -> None:
    await yak.build("app:app")

    with pytest.raises(YakException):  # noqa B908
        dep_file_path = tmp_path / "dep_file"
        await yak.build("app:app[dep_file]", f"--out={dep_file_path}")

        # If we get here, that means materialization did not fail.
        with open(dep_file_path, "rb") as f:
            dep_file = f.read()
            dep_file_hash = hashlib.sha1(dep_file).hexdigest()
            dep_file_len = len(dep_file)
            raise Exception(
                f"Misconfigured test, YAK_TEST_TOMBSTONED_DIGESTS to {dep_file_hash}:{dep_file_len}",
            )

    touch(yak, "app/other.h")
    await yak.build("app:app")

    await expect_exec_count(yak, 1)


@yak_test(data_dir="invalid_dep_files")
async def test_invalid_dep_files(yak: Yak) -> None:
    await yak.build(
        "//:lazy",
    )
    # Disable remote cache lookup so we actually check for local dep files
    await expect_failure(
        yak.build(
            "//:lazy",
            "-c",
            "test.seed=123",
            "--no-remote-cache",
        ),
        stderr_regex="Invalid line encountered in dep file",
    )

    await yak.debug("flush-dep-files")
    await yak.build("//:lazy")

    # Disable remote cache lookup so we actually check for local dep files
    await expect_failure(
        yak.build(
            "//:eager",
            "--eager-dep-files",
            "--no-remote-cache",
        ),
        stderr_regex="Invalid line encountered in dep file",
    )


@yak_test(data_dir="mismatched_outputs_dep_files")
async def test_mismatched_outputs_dep_files(yak: Yak) -> None:
    await yak.build("//:test", "-c", "test.prefix=foo", "-c", "test.suffix=bar")
    # Different output now, even though the command has not changed.
    await yak.build("//:test", "-c", "test.prefix=foo/bar", "-c", "test.suffix=")


async def _dep_file_uploads(yak: Yak) -> list[dict[str, Any]]:
    return await filter_events(
        yak, "Event", "data", "SpanEnd", "data", "DepFileUpload"
    )


async def _action_executions(yak: Yak) -> list[dict[str, Any]]:
    return await filter_events(
        yak, "Event", "data", "SpanEnd", "data", "ActionExecution"
    )


async def _dep_file_key_from_executions(yak: Yak) -> str:
    execs = await _action_executions(yak)
    assert len(execs) == 1
    return execs[0]["dep_file_key"]


async def _check_uploaded_dep_file_key(yak: Yak, dep_file_key: str) -> None:
    # YAK_TEST_SKIP_ACTION_CACHE_WRITE causes action result writes for dep files to always pass.
    # This is to allow testing without action cache write permission.
    dep_file_uploads = [
        upload for upload in await _dep_file_uploads(yak) if upload["success"]
    ]
    assert len(dep_file_uploads) == 1
    uploaded_key = dep_file_uploads[0]["remote_dep_file_key"]
    assert dep_file_key == uploaded_key


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files")
@env("YAK_LOG", "yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_uploads_same_key(yak: Yak) -> None:
    # Test all the cases where the remote dep file key should stay the same
    target = "root//:dep_files"
    tagged_used_file1 = yak.cwd / "used.1"  # Used for depfile 0
    tagged_used_file3 = yak.cwd / "used.3"  # Used for depfile 1
    assert tagged_used_file1.exists()
    assert tagged_used_file3.exists()

    target = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    ]

    # Check that building this target results in a dep file cache upload
    await yak.build(*target)

    key = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key)

    # Changing a tagged (associated with a dep file) input should not change the key
    # The remote dep file key only tracks the untagged inputs. The dep file cache is for checking whether
    # the output is the same despite a tagged file changing.
    tagged_used_file1.write_text("CHANGE")
    tagged_used_file3.write_text("CHANGE")
    await yak.build(*target)
    key_tagged_input_change = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key_tagged_input_change)
    assert key == key_tagged_input_change


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files")
@env("YAK_LOG", "yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_uploads_different_key(yak: Yak) -> None:
    # TODO: Mergebase is currently not set in this test.
    # Include it so we can test for the case where the mergebase differs

    keys_seen = []
    target = "root//:dep_files"
    untagged_file1 = yak.cwd / "untagged.1"
    assert untagged_file1.exists()
    targets_file = yak.cwd / "YAK.fixture"
    assert targets_file.exists()

    target = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    ]

    # Check that building this target results in a dep file cache upload
    await yak.build(*target)
    key = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key)
    keys_seen.append(key)

    # Modify the depfile name and check the new key is different
    targets_file.write_text(
        targets_file.read_text().replace(
            '"dep_file_name1",', '"dep_file_name1_modified",'
        )
    )
    await yak.build(*target)

    key_different_depfile_name = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key_different_depfile_name)
    assert key_different_depfile_name not in keys_seen
    keys_seen.append(key_different_depfile_name)

    # Modify the output name and check the new key is different
    targets_file.write_text(
        targets_file.read_text().replace(
            'out_name = "dep_files_out"', 'out_name = "dep_files_out_changed"'
        )
    )
    await yak.build(*target)
    key_different_out_name = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key_different_out_name)
    assert key_different_out_name not in keys_seen
    keys_seen.append(key_different_out_name)

    # Modify an untagged input and check the new key is different
    untagged_file1.write_text("CHANGE")
    await yak.build(*target)
    key_untagged_input_change = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key_untagged_input_change)
    assert key_untagged_input_change not in keys_seen
    keys_seen.append(key_untagged_input_change)


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files")
@env("YAK_LOG", "yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_dep_file_does_not_upload_when_allow_cache_upload_is_true(
    yak: Yak,
) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=false",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we don't do a dep file cache upload when allow_dep_file_cache_upload is false,
    # even though allow_cache_upload is true
    await yak.build(*target)
    uploads = await _dep_file_uploads(yak)
    assert len(uploads) == 0


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files")
@env("YAK_LOG", "yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
@env("YAK_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_only_do_cache_lookup_when_dep_file_upload_is_enabled(
    yak: Yak,
) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=false",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we don't do a dep file cache lookup when allow_dep_file_cache_upload is false
    await yak.build(*target)
    await check_no_cache_query(yak)

    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we do a dep file cache lookup when allow_dep_file_cache_upload is true
    await yak.build(*target)
    await check_cache_query(yak)


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files")
@env("YAK_LOG", "yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_remote_upload(yak: Yak) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that building on RE results in a dep file cache upload
    await yak.build(*target)
    key = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key)


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files", write_invocation_record=True)
@env("YAK_LOG", "yak_action_impl=debug,yak_execute_impl::executors::caching=debug")
@env("YAK_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_cache_hit_upload(yak: Yak) -> None:
    target = [
        "root//:dep_files",
        "--remote-only",
        "-c",
        # Ensure we don't get a dep file cache hit
        "test.remote_dep_file_cache_enabled=false",
    ]

    # Build on RE to make sure action cache is populated
    await yak.build(*target)
    await yak.kill()

    # Check for action cache hit and dep file cache upload
    res = await yak.build(
        *target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
    )
    what_ran = await read_what_ran(yak)
    assert what_ran[0]["reproducer"]["executor"] == "Cache"
    assert len(what_ran) == 1
    key = await _dep_file_key_from_executions(yak)
    await _check_uploaded_dep_file_key(yak, key)

    invocation_record = res.invocation_record()

    assert invocation_record["dep_file_upload_count"] == 1
    assert (
        invocation_record["dep_file_upload_count"]
        == invocation_record["dep_file_upload_attempt_count"]
    )

    # Simulate 'user' build, with action cache hit from previous build and dep file cache checking enabled.
    await yak.clean()
    await yak.build(
        "root//:dep_files",
        "--remote-only",
        "-c",
        "test.remote_dep_file_cache_enabled=true",
        "-c",
        "test.allow_dep_file_cache_upload=false",
    )
    await check_execution_kind(yak, [ACTION_EXECUTION_KIND_ACTION_CACHE])
    uploads = await _dep_file_uploads(yak)
    # Ensure no dep file uploads are attempted for cache hits with dep file cache checking enabled, but dep file uploads disabled.
    assert len(uploads) == 0


@yak_test(data_dir="upload_dep_files")
async def test_re_dep_file_uploads_failed_action(yak: Yak) -> None:
    # If the action failed, we should not attempt to upload to cache even if it's configured to
    target = [
        "root//:dep_files_fail",
        "-c",
        "test.allow_dep_file_cache_upload=true",
    ]
    await expect_failure(
        yak.build(
            *target,
            "--no-remote-cache",
            "--local-only",
        ),
        stderr_regex="Failing on purpose",
    )
    # Assert cache upload was not attempted
    what_ran = await read_what_ran(yak, "--emit-cache-queries")
    for what in what_ran:
        assert "CacheQuery" != what["reproducer"]["executor"]


async def check_remote_dep_file_cache_query_took_place(yak: Yak) -> str:
    what_ran = await read_what_ran(yak, "--emit-cache-queries")
    assert "CacheQuery" == what_ran[0]["reproducer"]["executor"]
    return what_ran[0]["reproducer"]["details"]["digest"]


@yak_test(data_dir="upload_dep_files")
@env(
    "YAK_LOG",
    "yak_execute_impl::executors::caching=debug,yak_execute_impl::executors::action_cache=debug,yak_action_impl=debug",
)
# Disable the regular action cache query so that we actually hit the remote dep file cache query.
@env("YAK_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_re_dep_file_query_change_tagged_unused_file(yak: Yak) -> None:
    target = "root//:dep_files"
    # Tagged for depfile0, and exists in depfile0
    tagged_used_file1 = yak.cwd / "used.1"
    # Tagged for depfile0, but does NOT exist in depfile0
    tagged_unused = yak.cwd / "unused.1"
    assert tagged_used_file1.exists()
    assert tagged_unused.exists()

    target_upload_enabled = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        "test.cache_buster=tagged_unused_file_test",
        "--local-only",
    ]

    target_upload_enabled_with_action_definition_change = target_upload_enabled + [
        "-c",
        "test.allow_cache_upload=true",
    ]

    # Build it once with cache upload (cache upload will fail locally)
    result = await yak.build(*target_upload_enabled, "--no-remote-cache")
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    # Build the target again. This will either result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is
    #    flushed) This can occur if the action definition changes, because the new remote dep file
    #    can only be uploaded by a job with the correct permissions, so it will run locally until
    #    that takes place.
    await yak.debug("flush-dep-files")
    result = await yak.build(*target_upload_enabled_with_action_definition_change)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    await check_remote_dep_file_cache_query_took_place(yak)
    execution_kind = await _get_execution_kind(yak)
    was_cache_hit = "Cache hits: 100%" in result.stderr
    assert (
        was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE
    ) or (not was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_LOCAL)
    expected_dep_file_match_events = [
        MatchDepFilesEvent(
            remote_cache=False, checking_filtered_inputs=False
        ),  # Initial local dep file cache lookup for an identical action
    ]

    if execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE:
        expected_dep_file_match_events.append(
            MatchDepFilesEvent(remote_cache=True, checking_filtered_inputs=True)
        )  # Remote dep file cache hit verification

    # Check the MatchDepFiles events
    await check_match_dep_files_events(yak, expected_dep_file_match_events)

    # # Change a file that is tracked by a dep file but shows up as unused, we get a local dep file cache hit
    # # as that is checked first.
    tagged_unused.write_text(random_string())
    result = await yak.build(*target_upload_enabled)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    execution_kind = await _get_execution_kind(yak)
    assert execution_kind == ACTION_EXECUTION_KIND_LOCAL_DEP_FILE

    # Change a file that is tracked by a dep file but shows up as unused, this will again result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is flushed)
    await yak.debug("flush-dep-files")
    tagged_unused.write_text(random_string())
    result = await yak.build(*target_upload_enabled)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    await check_remote_dep_file_cache_query_took_place(yak)
    execution_kind = await _get_execution_kind(yak)
    was_cache_hit = "Cache hits: 100%" in result.stderr
    assert (
        was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE
    ) or (not was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_LOCAL)

    # Check the MatchDepFiles events
    await check_match_dep_files_events(yak, expected_dep_file_match_events)


@yak_test(data_dir="upload_dep_files")
@env(
    "YAK_LOG",
    "yak_execute_impl::executors::caching=debug,yak_execute_impl::executors::action_cache=debug,yak_action_impl=debug",
)
# Disable the regular action cache query so that we actually hit the remote dep file cache query.
@env("YAK_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_re_dep_file_query_change_tagged_used_file(yak: Yak) -> None:
    target = "root//:dep_files"
    # Tagged for depfile0, and exists in depfile0
    tagged_used_file1 = yak.cwd / "used.1"
    # Tagged for depfile0, but does NOT exist in depfile0
    tagged_unused = yak.cwd / "unused.1"
    assert tagged_used_file1.exists()
    assert tagged_unused.exists()

    target_upload_enabled = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "--local-only",
    ]

    # Build it once with cache upload (cache upload will fail locally)
    result = await yak.build(*target_upload_enabled, "--no-remote-cache")
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    # Change a file that is tracked by a dep file and shows up as used (ends up listed in the dep file).
    # Build the target again. This will either result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation (which fails)
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is flushed)
    # Either way, it should be executed locally
    await yak.debug("flush-dep-files")
    used1_modified_str = f"used1({random_string()})"
    tagged_used_file1.write_text(f"{used1_modified_str}\n")
    result = await yak.build(*target_upload_enabled)
    await check_remote_dep_file_cache_query_took_place(yak)
    await check_execution_kind(yak, [ACTION_EXECUTION_KIND_LOCAL])
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == f"{used1_modified_str}\nused2\nused3\n"


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@yak_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_flush_dep_files(yak: Yak) -> None:
    # Make sure that we build locally
    args = ["app:app", "--no-remote-cache", "--local-only"]
    await yak.build(*args)
    await expect_exec_count(yak, 1)

    await yak.debug("flush-dep-files", "--retain-local")

    # //app:app doesn't use other.h and
    # dep file should still be present
    # since we retained local dep files
    touch(yak, "app/other.h")
    await yak.build(*args)
    await expect_only_dep_file_hit(yak)

    await yak.debug("flush-dep-files")

    # all dep files are gone, so we have
    # to rebuild.
    touch(yak, "app/other.h")
    await yak.build(*args)
    await expect_exec_count(yak, 1)


async def run_test_input_cannot_be_normalized(
    yak: Yak, allow_soft_errors: bool
) -> None:
    target = "root//:input_cannot_be_normalized"
    tagged_unused = yak.cwd / "unused.1"
    assert tagged_unused.exists()

    # We query cache before we query dep file. Disable remote cache to make
    # sure that for the last build what-ran doesn't return cached entry.
    args = [target, "--no-remote-cache"]
    await yak.build(*args)
    await expect_exec_count(yak, 1)

    # We should get a dep file cache hit, but we don't because the input cannot be normalized.
    tagged_unused.write_text(random_string())
    if allow_soft_errors:
        await yak.build(*args)
        await expect_exec_count(yak, 1)
    else:
        await expect_failure(
            yak.build(*args),
            stderr_regex="Path.*cannot be normalized for dep-files because it has two path segments that look like a content-based hash!",
        )


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files", allow_soft_errors=False)
async def test_input_cannot_be_normalized_and_hard_error(yak: Yak) -> None:
    await run_test_input_cannot_be_normalized(yak, False)


@pytest.mark.remote_execution
@yak_test(data_dir="upload_dep_files", allow_soft_errors=True)
async def test_input_cannot_be_normalized(yak: Yak) -> None:
    await run_test_input_cannot_be_normalized(yak, True)


@yak_test(data_dir="invalid_dep_files")
async def test_two_outputs_tagged_as_dep_file(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:two_outputs_tagged_as_dep_file"),
        stderr_regex="`dep_files` value with key `deps` has an invalid count of associated outputs. Expected 1, got 2",
    )


@yak_test(data_dir="invalid_dep_files")
async def test_no_outputs_tagged_as_dep_file(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:no_outputs_tagged_as_dep_file"),
        stderr_regex="`dep_files` value with key `deps` has an invalid count of associated outputs. Expected 1, got 0",
    )


@yak_test(data_dir="invalid_dep_files")
async def test_same_tag_for_multiple_labels(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:same_tag_for_multiple_labels"),
        stderr_regex="`dep_files` with keys `deps` and `deps2` are using the same tag",
    )


@yak_test(data_dir="invalid_dep_files")
async def test_input_tagged_multiple_times(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//:input_tagged_multiple_times"),
        stderr_regex="Dep-files input.*input_tagged_multiple_times.txt.*is tagged with multiple tags relevant for dep-files: `deps1` and `deps2`",
    )


async def _build_canonical_input(
    yak: Yak, options: dict[str, str], expected_kind: int
) -> BuildResult:
    flags = [
        flag for key, value in options.items() for flag in ("-c", f"test.{key}={value}")
    ]
    result = await yak.build(
        "root//app:canonical_json",
        "root//app:canonical_json[json]",
        "--local-only",
        "--no-remote-cache",
        *flags,
    )
    assert (
        await get_last_execution_kind(
            yak, category="canonical_json_consumer", target_name="canonical_json"
        )
        == expected_kind
    )
    return result


def _canonical_input_output(result: BuildResult) -> tuple[str, str]:
    report = result.get_build_report()
    return (
        report.output_for_target("root//app:canonical_json").read_text(),
        report.output_for_target("root//app:canonical_json", "json").read_text(),
    )


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize("content_paths", ["true", "false"])
@pytest.mark.parametrize(
    "input_format, placement",
    [
        ("json", "direct"),
        ("json", "without_inputs"),
        ("args", "direct"),
        ("args", "no_allow_args"),
        ("args", "nested"),
        ("args", "without_inputs"),
    ],
)
async def test_canonical_input_dep_file(
    yak: Yak, content_paths: str, input_format: str, placement: str
) -> None:
    options = {
        "use_content_based_paths": content_paths,
        "canonical_placement": placement,
        "canonical_format": input_format,
    }
    output, original_json = _canonical_input_output(
        await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    )
    assert output == "optionusedextra"

    options["canonical_unused"] = "unused-changed"
    output, changed_json = _canonical_input_output(
        await _build_canonical_input(
            yak, options, ACTION_EXECUTION_KIND_LOCAL_DEP_FILE
        )
    )
    assert output == "optionusedextra"
    assert (original_json != changed_json) == (content_paths == "true")

    for key, value, expected in [
        ("canonical_used", "used-changed", "optionused-changedextra"),
        ("canonical_untagged", "extra-changed", "optionused-changedextra-changed"),
        ("canonical_option", "new-option", "new-optionused-changedextra-changed"),
        ("canonical_reverse", "true", "new-optionunused-changedextra-changed"),
        ("canonical_repeat", "true", "new-optionunused-changedextra-changed"),
    ]:
        options[key] = value
        output, _ = _canonical_input_output(
            await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
        )
        assert output == expected


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize("content_paths", ["true", "false"])
async def test_canonical_args_executable(yak: Yak, content_paths: str) -> None:
    options = {
        "canonical_format": "args",
        "canonical_option": "executable-bit",
        "use_content_based_paths": content_paths,
    }
    for executable in ["false", "true", "false"]:
        options["canonical_executable"] = executable
        output, _ = _canonical_input_output(
            await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
        )
        assert output == str(executable == "true")


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_absolute_pretty(yak: Yak, input_format: str) -> None:
    options = {
        "canonical_absolute": "true",
        "canonical_pretty": "true",
        "canonical_format": input_format,
    }
    output, original_json = _canonical_input_output(
        await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    )
    assert output == "optionusedextra"
    if input_format == "json":
        assert Path(json.loads(original_json)["inputs"][0]).is_absolute()
        assert "\n" in original_json
    else:
        assert Path(shlex.split(original_json)[4]).is_absolute()
        assert "\n" not in original_json
    options["canonical_unused"] = "changed"
    output, changed_json = _canonical_input_output(
        await _build_canonical_input(
            yak, options, ACTION_EXECUTION_KIND_LOCAL_DEP_FILE
        )
    )
    assert output == "optionusedextra"
    assert original_json != changed_json


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_ordinary_occurrence(
    yak: Yak, input_format: str
) -> None:
    options = {"canonical_placement": "ordinary", "canonical_format": input_format}
    await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    options["canonical_unused"] = "changed"
    # An ordinary occurrence of the same file still requires its physical digest to match.
    output, _ = _canonical_input_output(
        await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    )
    assert output == "optionusedextra"


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize(
    "placement, error",
    [
        (
            "with_inputs",
            "`with_inputs = True` cannot be combined with `dep_files_fingerprint_using_canonical_paths = True`: "
            "the fingerprint already forwards inputs with their original tags",
        ),
        ("placeholder", "cannot be combined with placeholder output"),
        ("command_line", "DepFileFingerprint"),
        ("invalid_descriptor", "DepFileFingerprint"),
    ],
)
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_invalid_placement(
    yak: Yak, placement: str, error: str, input_format: str
) -> None:
    await expect_failure(
        yak.build(
            "root//app:canonical_json",
            "-c",
            f"test.canonical_placement={placement}",
            "-c",
            f"test.canonical_format={input_format}",
            "-c",
            "test.use_content_based_paths=false",
        ),
        stderr_regex=error,
    )


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={"yak": {"sqlite_dep_file_state": "true"}},
)
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_persisted_dep_file(
    yak: Yak, input_format: str
) -> None:
    await _build_canonical_input(
        yak, {"canonical_format": input_format}, ACTION_EXECUTION_KIND_LOCAL
    )
    await yak.kill()
    await _build_canonical_input(
        yak,
        {"canonical_format": input_format},
        ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE,
    )
    # Reloaded entries have no live filtered-input signatures. The first changed build
    # executes; subsequent unused changes can use the new live dep-file entry.
    await _build_canonical_input(
        yak,
        {"canonical_format": input_format, "canonical_unused": "changed"},
        ACTION_EXECUTION_KIND_LOCAL,
    )
    output, _ = _canonical_input_output(
        await _build_canonical_input(
            yak,
            {"canonical_format": input_format, "canonical_unused": "changed-again"},
            ACTION_EXECUTION_KIND_LOCAL_DEP_FILE,
        )
    )
    assert output == "optionusedextra"
    output, _ = _canonical_input_output(
        await _build_canonical_input(
            yak,
            {"canonical_format": input_format, "canonical_option": "new-option"},
            ACTION_EXECUTION_KIND_LOCAL,
        )
    )
    assert output == "new-optionusedextra"


@yak_test(
    data_dir="dep_files",
    write_invocation_record=True,
    skip_for_os=["windows"],
    extra_yak_config={
        "yak_hydration": {
            "enable_paging": "true",
            "page_out_on_idle": "true",
            "page_out_min_free_disk_gb": "0",
        }
    },
)
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_paged_analysis(yak: Yak, input_format: str) -> None:
    options = {"canonical_unused_src": "true", "canonical_format": input_format}
    await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    await yak.debug("hydration", "page-out")
    (yak.cwd / "app/other.h").write_text("changed unused source")
    result = await _build_canonical_input(
        yak, options, ACTION_EXECUTION_KIND_LOCAL_DEP_FILE
    )
    assert result.invocation_record()["page_in_count"] > 0
    output, _ = _canonical_input_output(result)
    assert output == "optionusedextra"


@yak_test(
    data_dir="dep_files",
    skip_for_os=["windows"],
    extra_yak_config={
        "build": {"execution_platforms": "root//app:canonical_platforms"}
    },
)
@env("YAK_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
@pytest.mark.parametrize("input_format", ["json", "args"])
async def test_canonical_input_remote_dep_file_key(
    yak: Yak, input_format: str
) -> None:
    options = {"canonical_option": random_string(), "canonical_format": input_format}

    async def build() -> tuple[str, str, int | None]:
        flags = [
            flag
            for key, value in options.items()
            for flag in ("-c", f"test.{key}={value}")
        ]
        result = await yak.build("root//app:canonical_json", "--local-only", *flags)
        queries = [
            entry["reproducer"]["details"]["digest"]
            for entry in await read_what_ran(
                yak,
                "--emit-cache-queries",
                "--filter-category",
                "canonical_json_consumer",
            )
            if entry["reproducer"]["executor"] == "CacheQuery"
        ]
        assert len(queries) == 1
        output = (
            result.get_build_report()
            .output_for_target("root//app:canonical_json")
            .read_text()
        )
        kind = await get_last_execution_kind(yak, category="canonical_json_consumer")
        return queries[0], output, kind

    initial_key, output, _ = await build()
    assert output == options["canonical_option"] + "usedextra"
    await yak.debug("flush-dep-files")
    options["canonical_unused"] = "changed"
    unused_key, output, kind = await build()
    assert unused_key == initial_key
    assert output == options["canonical_option"] + "usedextra"
    # Developer machines may not have permission to populate the remote cache.
    assert kind in (
        ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE,
        ACTION_EXECUTION_KIND_LOCAL,
    )

    await yak.debug("flush-dep-files")
    options["canonical_used"] = "used-changed"
    used_key, output, kind = await build()
    assert used_key == initial_key
    assert output == options["canonical_option"] + "used-changedextra"
    assert kind == ACTION_EXECUTION_KIND_LOCAL

    await yak.debug("flush-dep-files")
    options["canonical_option"] += "-changed"
    changed_key, output, kind = await build()
    assert changed_key != initial_key
    assert output == options["canonical_option"] + "used-changedextra"
    assert kind == ACTION_EXECUTION_KIND_LOCAL


@yak_test(data_dir="dep_files", skip_for_os=["windows"])
@pytest.mark.parametrize("delimiter", ["true", "false"])
async def test_canonical_args_quoting(yak: Yak, delimiter: str) -> None:
    option = "option with 'quotes' and\na newline"
    options = {
        "canonical_format": "args",
        "canonical_pretty": delimiter,
        "canonical_option": option,
    }
    output, args = _canonical_input_output(
        await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    )
    assert output == option + "usedextra"
    assert shlex.split(args)[0] == option
    options["canonical_unused"] = "changed"
    await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL_DEP_FILE)
    options["canonical_empty_option"] = "true"
    output, args = _canonical_input_output(
        await _build_canonical_input(yak, options, ACTION_EXECUTION_KIND_LOCAL)
    )
    assert output == "usedextra"
    assert shlex.split(args)[0] == ""
