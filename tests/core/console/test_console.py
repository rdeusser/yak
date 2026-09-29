# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import collections
import json
import os
import re
from pathlib import Path
from typing import Any

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test, env


def fixture(name: str) -> str:
    p = Path(__file__).resolve().parent / "fixtures" / f"{name}.proto"
    return str(p)


@yak_test()
async def test_console_facts(yak: Yak) -> None:
    res = await yak.log(
        "replay",
        fixture("my_genrule0"),
    )
    assert re.search("Network: .*([0-9.]+)([KMG]?)B", res.stderr) is not None
    assert "Cache hits: 100%" in res.stderr
    assert "Commands: 1 (cached: 1, remote: 0, local: 0)" in res.stderr


@yak_test()
async def test_console_facts_no_repo(yak: Yak) -> None:
    res = await yak.log(
        "replay",
        fixture("my_genrule0"),
        rel_cwd=Path(os.path.relpath("/", yak.cwd)),
    )
    assert re.search("Network: .*([0-9.]+)([KMG]?)B", res.stderr) is not None
    assert "Cache hits: 100%" in res.stderr
    assert "Commands: 1 (cached: 1, remote: 0, local: 0)" in res.stderr


@yak_test()
async def test_super_console_facts(yak: Yak) -> None:
    res = await yak.log("replay", fixture("my_genrule0"))
    assert re.search("Network: .*([0-9.]+)([KMG]?)B", res.stderr) is not None
    assert "Cache hits: 100%" in res.stderr
    assert "Commands: 1" in res.stderr


@yak_test()
async def test_whatran(yak: Yak) -> None:
    res = await yak.log(
        "what-ran",
        fixture("my_genrule0"),
    )
    assert "cache" in res.stdout
    assert (
        "317ac52ba10a8231a17a66dd40ffbf825a274ba63ca034653c289c5e4ca20e05:141"
        in res.stdout
    )


@yak_test()
async def test_whatran_no_repo(yak: Yak) -> None:
    res = await yak.log(
        "what-ran",
        fixture("my_genrule0"),
        rel_cwd=Path(os.path.relpath("/", yak.cwd)),
    )
    assert "cache" in res.stdout
    assert (
        "317ac52ba10a8231a17a66dd40ffbf825a274ba63ca034653c289c5e4ca20e05:141"
        in res.stdout
    )


@yak_test()
async def test_file_watcher_span_depth(yak: Yak) -> None:
    """
    We show spans up to depth 2 in the console. We should make sure that spans
    whose runtime depends on external tools (i.e. the file watcher) are
    displayed.
    """
    await yak.build()
    log = await yak.log("show")

    depths = collections.defaultdict(int)
    file_watcher_span = None

    for line in log.stdout.splitlines():
        line = json.loads(line)
        event = line.get("Event")
        if event is None:
            continue

        span = _get(event, "data", "SpanStart", "data")
        if span is None:
            continue

        # This event is relevant to us, but it's also not shown, so it means
        # its children are roots.
        if "DiceCriticalSection" in span:
            depth = -1
        else:
            depth = depths[event["parent_id"]] + 1

        depths[event["span_id"]] = depth

        if _get(event, "data", "SpanStart", "data", "FileWatcher"):
            file_watcher_span = event

    assert file_watcher_span is not None, "Did not find FileWatcher span"
    assert depths[file_watcher_span["span_id"]] <= 2


@yak_test()
async def test_stale_snapshot(yak: Yak, tmp_path: Path) -> None:
    original = fixture("my_genrule0")
    log = (await yak.log("show", original)).stdout

    # Now we're going to make a new log where we just delay the last event by
    # some amount of time.
    lines = log.splitlines()

    # Last event (last line is command result).
    last = lines[-2]
    last = json.loads(last)
    last["Event"]["timestamp"][0] += 20
    lines[-2] = json.dumps(last)

    logfile = tmp_path / "test.json-lines"

    with open(logfile, "w") as f:
        f.write("\n".join(lines))

    stale_message = "Resource usage: <snapshot is stale>"

    # Check it's there.
    res = await yak.log("replay", str(logfile))
    assert stale_message in res.stderr

    # Check it's not in the original one.
    res = await yak.log("replay", original)
    assert stale_message not in res.stderr


def _get(data: dict[str, Any], *key: str) -> dict[str, Any] | None:
    for k in key:
        res = data.get(k)
        if res is None:
            return None
        else:
            data = res
    return data


@yak_test()
async def test_super_console_changes(yak: Yak) -> None:
    res = await yak.log("replay", fixture("my_genrule1"))
    assert "File changed: root//dir1/file1" in res.stderr
    assert "Directory changed: root//dir1/sub" in res.stderr


# The daemon measures its memory with RSS, which it does not read on macOS,
# and with jemalloc statistics, which Cargo builds do not collect. On macOS it
# has no measurement, so it never warns.
@yak_test(
    extra_yak_config={
        "yak_system_warning": {
            "memory_pressure_threshold_percent": "1",
        },
    },
    skip_for_os=["darwin"],
)
@env("YAK_TEST_FAKE_SYSTEM_TOTAL_MEMORY", "1000")
async def test_system_memory_exceeded_warning(yak: Yak) -> None:
    res = await yak.build("//:slow", "--console=simple")
    assert "High memory pressure" in res.stderr
