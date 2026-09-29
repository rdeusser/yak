# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import sys
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import env, yak_test
from e2e_util.helper.http_server import sha1_hex, StaticHttpServer
from e2e_util.helper.utils import (
    filter_events,
    random_string,
    replace_in_file,
)

# The CAS names content by the daemon's digest algorithm, and the download tests declare
# SHA-1 checksums, as upstream's tests do. yak defaults to SHA-256.
sha1_digests = env("YAK_DEFAULT_DIGEST_ALGORITHM", "SHA1")


def watchman_dependency_linux_only() -> bool:
    return sys.platform == "linux"


@yak_test(data_dir="modify_deferred_materialization")
async def test_modify_input_source(yak: Yak) -> None:
    await yak.build("//:urandom_dep")

    targets_file = yak.cwd / "YAK.fixture"

    # Change the label in Targets.
    replace_in_file("__NOT_A_REAL_LABEL__", "yak_test_local_exec", file=targets_file)

    await yak.build("//:urandom_dep")


@yak_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_modify_dep_materialization(yak: Yak) -> None:
    target = "//:check"

    # Build, expect the symlink to work. We'll materialize the first time.

    result = await yak.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT"

    # Build again, expect the symlink to work. We'll materialize just deps.

    with open(yak.cwd / "text", "w", encoding="utf-8") as f:
        f.write("TEXT2")

    result = await yak.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT2"

    # Build again, expect the symlink to work. We'll materialize just deps
    # again. However this time our state is a little different since the
    # previous future was a check-deps only future.

    with open(yak.cwd / "text", "w", encoding="utf-8") as f:
        f.write("TEXT3")

    result = await yak.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT3"


@pytest.mark.remote_execution
@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("YAK_LOG", "yak_execute_impl::materializers=trace")
async def test_matching_artifact_optimization(yak: Yak) -> None:
    target = "root//:copy"
    result = await yak.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    # In this case, modifying `hidden` does not change the output, so the output should not
    # need to be rematerialized
    with open(yak.cwd / "hidden", "w", encoding="utf-8") as f:
        f.write("HIDDEN2")

    result = await yak.build(target)
    # Check output still exists
    assert result.get_build_report().output_for_target(target).exists()
    # Check that materializer did not report any rematerialization
    assert "already materialized, updating deps only" in result.stderr
    assert "materialize artifact" not in result.stderr

    # In this case, modifying `src` changes the output, so the output should be rematerialized
    with open(yak.cwd / "src", "w", encoding="utf-8") as f:
        f.write("SRC2")

    result = await yak.build(target)
    # Check output still exists
    output = result.get_build_report().output_for_target(target)
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == "SRC2"


@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
async def test_cache_directory_cleanup(yak: Yak) -> None:
    # sqlite materializer state is already enabled
    cache_dir = Path(yak.cwd, "yak-out", "v2", "cache")
    materializer_state_dir = cache_dir / "materializer_state"
    materializer_state_dir.mkdir(parents=True)
    incremental_state_dir = cache_dir / "incremental_state"
    incremental_state_dir.mkdir(parents=True)
    command_hashes_dir = cache_dir / "command_hashes"
    command_hashes_dir.mkdir(parents=True)

    # Need to run a command to start the daemon.
    await yak.audit_config()

    cache_dir_listing = sorted(list(cache_dir.iterdir()))
    assert cache_dir_listing == [incremental_state_dir, materializer_state_dir]

    await yak.kill()
    disable_sqlite_materializer_state(yak)
    await yak.audit_config()

    cache_dir_listing = list(cache_dir.iterdir())
    assert cache_dir_listing == [incremental_state_dir]


@pytest.mark.remote_execution
@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("YAK_LOG", "yak_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_matching_artifact_optimization(
    yak: Yak,
) -> None:
    # sqlite materializer state is already enabled
    target = "root//:copy"
    res = await yak.build(target)
    # Check output is correctly materialized
    assert res.get_build_report().output_for_target(target).exists()

    await yak.kill()

    res = await yak.build(target)
    # Check that materializer did not report any rematerialization
    assert "already materialized, updating deps only" in res.stderr, res.stderr
    assert "materialize artifact" not in res.stderr

    await yak.kill()

    # In this case, modifying `src` changes the output, so the output should be rematerialized
    with open(yak.cwd / "src", "w", encoding="utf-8") as f:
        f.write("SRC2")

    res = await yak.build(target)
    # Check output still exists
    output = res.get_build_report().output_for_target(target)
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == "SRC2"


@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@sha1_digests
async def test_download_file_not_repeated_after_restart(
    yak: Yak,
) -> None:
    # Fresh content, so the CAS cannot already have it and the download really happens.
    content = random_string().encode()
    async with StaticHttpServer({"/file": content}) as server:
        target = "root//:download"
        configs = [
            "-c",
            f"test.url={server.url('/file')}",
            "-c",
            f"test.sha1={sha1_hex(content)}",
        ]

        res = await yak.build(target, *configs)
        output = res.get_build_report().output_for_target(target)
        assert output.read_bytes() == content
        assert server.count("GET", "/file") == 1

        # The sqlite materializer state tells the new daemon the file is already there.
        await yak.kill()
        await yak.build(target, *configs)
        assert output.read_bytes() == content
        assert server.count("GET", "/file") == 1


@pytest.mark.remote_execution
@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("YAK_LOG", "yak_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_disabled(
    yak: Yak,
) -> None:
    disable_sqlite_materializer_state(yak)

    target = "root//:copy"
    result = await yak.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    await yak.kill()

    result = await yak.build(target)
    # Check that materializer did have to rematerialize the same artifact
    assert "already materialized, updating deps only" not in result.stderr
    assert "materialize artifact" in result.stderr


@yak_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("YAK_LOG", "yak_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_yakconfig_version_change(
    yak: Yak,
) -> None:
    # sqlite materializer state is already enabled
    target = "root//:copy"
    result = await yak.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    await yak.kill()

    # Bump the yakconfig version of sqlite materializer state to invalidate the existing sqlite db
    replace_in_file(
        "sqlite_materializer_state_version = 0",
        "sqlite_materializer_state_version = 1",
        yak.cwd / ".yakconfig",
    )

    # just starting the yak daemon should delete the sqlite materializer state
    await yak.audit_config()


@yak_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],
)
async def test_materialization_spans_have_parent_id(yak: Yak) -> None:
    """Materialization spans should be parented to the span that triggered them,
    not appear as root spans with parent_id == 0."""
    await yak.build("//:check")

    materialization_events = await filter_events(
        yak,
        "Event",
        "data",
        "SpanStart",
        "data",
        "Materialization",
        return_root=True,
    )

    assert len(materialization_events) > 0, "Expected at least one Materialization span"
    for event in materialization_events:
        assert event["Event"]["parent_id"] != 0, (
            f"Materialization span has parent_id == 0 (no parent): {event}"
        )


@yak_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],
)
async def test_materializer_command_events_have_parent_id(yak: Yak) -> None:
    """MaterializerCommand instant events emitted on the synchronous command
    processing thread should be parented to the span that sent the command,
    not appear as root events with parent_id == 0.  This requires
    verbose_materializer_event_log = true in .yakconfig."""
    await yak.build("//:check")

    command_events = await filter_events(
        yak,
        "Event",
        "data",
        "Instant",
        "data",
        "MaterializerCommand",
        return_root=True,
    )

    assert len(command_events) > 0, (
        "Expected at least one MaterializerCommand instant event "
        "(is verbose_materializer_event_log enabled?)"
    )
    for event in command_events:
        assert event["Event"]["parent_id"] != 0, (
            f"MaterializerCommand event has parent_id == 0 (no parent): {event}"
        )


def disable_sqlite_materializer_state(yak: Yak) -> None:
    config_file = yak.cwd / ".yakconfig"
    replace_in_file(
        "sqlite_materializer_state = true",
        "sqlite_materializer_state = false",
        file=config_file,
    )


@pytest.mark.remote_execution
@yak_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_debug_materialize(yak: Yak) -> None:
    result = await yak.build("//:remote_text", "--materializations=None")
    out = result.get_build_report().output_for_target(
        "root//:remote_text", rel_path=True
    )
    assert not Path(yak.cwd, out).exists()

    await yak.debug("materialize", str(out))
    assert Path(yak.cwd, out).exists()
