# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import contextlib
import hashlib
import json
import random
import re
import socket
import string
import sys
import typing
from dataclasses import dataclass
from pathlib import Path

import psutil
from aiohttp import web
from e2e_util.api.buck import Buck
from e2e_util.api.buck_result import InvocationRecord


def daemon_is_alive(pid: int) -> bool:
    return psutil.pid_exists(pid)


@dataclass
class ServedFile:
    """ServedFile describes a file that `serve_file` serves over HTTP."""

    url: str
    sha1: str
    sha256: str
    size: int


@contextlib.asynccontextmanager
async def serve_file(content: bytes) -> typing.AsyncIterator[ServedFile]:
    """Serves `content` at `/download` on 127.0.0.1 until the context exits.

    Download tests use it in place of a server on the internet.
    """

    async def handle(request: web.Request) -> web.Response:
        return web.Response(body=content)

    app = web.Application()
    app.router.add_get("/download", handle)
    runner = web.AppRunner(app, access_log=None)
    await runner.setup()
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
        try:
            await web.SockSite(runner, sock).start()
            yield ServedFile(
                url=f"http://127.0.0.1:{port}/download",
                sha1=hashlib.sha1(content).hexdigest(),
                sha256=hashlib.sha256(content).hexdigest(),
                size=len(content),
            )
        finally:
            await runner.cleanup()


def configure_served_file(buck: Buck, served: ServedFile) -> None:
    """Appends the URL and checksums of `served` to the project's .yakconfig.

    Test data reads them as `test.download_url`, `test.download_sha1`, and
    `test.download_sha256`.
    """
    with open(buck.cwd / ".yakconfig", "a") as f:
        f.write(
            "\n[test]\n"
            f"  download_url = {served.url}\n"
            f"  download_sha1 = {served.sha1}\n"
            f"  download_sha256 = {served.sha256}\n"
        )


def replace_in_file(old: str, new: str, file: Path, encoding: str = "utf-8") -> None:
    """Replace every occurrence of `old` in `file`; `old` must occur at least once."""
    with open(file, encoding=encoding) as f:
        file_content = f.read()
    assert old in file_content, f"{old!r} not found in {file}"
    file_content = file_content.replace(old, new)
    with open(file, "w", encoding=encoding) as f:
        f.write(file_content)


async def read_what_ran(buck: Buck, *args) -> typing.List[typing.Dict[str, typing.Any]]:
    out = await buck.log("what-ran", "--format", "json", *args)
    out = [line.strip() for line in out.stdout.splitlines()]
    out = [json.loads(line) for line in out if line]
    return out


def timestamp_ms(s: int, ns: int) -> int:
    f = int(ns / 1000000)
    assert f < 1000
    return s * 1000 + f


async def read_timestamps(buck: Buck, *args) -> typing.List[int]:
    log = (await buck.log("show")).stdout.strip().splitlines()
    return [
        timestamp_ms(*json.loads(line)["Event"]["timestamp"])
        for line in log
        if json_get(line, *args) is not None
    ]


def is_running_on_linux() -> bool:
    return sys.platform == "linux"


def is_running_on_mac() -> bool:
    return sys.platform == "darwin"


def is_running_on_windows() -> bool:
    return sys.platform == "win32"


def get_targets_from_what_ran(
    what_ran: typing.List[typing.Dict[str, typing.Any]],
) -> typing.Set[typing.Tuple[str, str]]:
    targets = set()

    for entry in what_ran:
        m = re.match(r"^(.*?)( \((.*?)\))?( \((.*?)\))?$", entry["identity"])
        assert m is not None
        rule, category = m.group(1), m.group(5)
        targets.add((rule, category))

    return targets


async def expect_exec_count(buck: Buck, n: int) -> None:
    out = await read_what_ran(buck)
    assert len(out) == n, "unexpected actions: %s" % (out,)


async def filter_events(
    buck: Buck,
    *args: str,
    rel_cwd: typing.Optional[Path] = None,
    return_root: bool = False,
) -> typing.List[typing.Any]:
    log = (await buck.log("show", rel_cwd=rel_cwd)).stdout.strip().splitlines()
    found = []
    for line in log:
        e = json_get(line, *args, return_root_on_match=return_root)
        if e is None:
            continue
        found.append(e)
    return found


def json_get(data: str, *key: str, return_root_on_match: bool = False) -> typing.Any:
    root = json.loads(data)
    cur = root

    for k in key:
        cur = cur.get(k)
        if cur is None:
            return None

    return root if return_root_on_match else cur


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))


def replace_hashes(strings: typing.List[str]) -> typing.List[str]:
    return [replace_hash(s) for s in strings]


def replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


def replace_digest(s: str) -> str:
    return re.sub(r"\b(?:[0-9a-f]{40}|[0-9a-f]{64}):[0-9]+\b", "<DIGEST>", s)


def read_invocation_record(record: Path) -> InvocationRecord:
    return InvocationRecord(record)


async def get_last_execution_kind(
    buck: Buck,
    category: typing.Optional[str] = None,
    excluded_execution_kinds: typing.Optional[typing.List[int]] = None,
    target_name: typing.Optional[str] = None,
) -> typing.Optional[int]:
    if excluded_execution_kinds is None:
        excluded_execution_kinds = []
    action_executions = await filter_events(
        buck,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
    )
    for action_execution in reversed(action_executions):
        execution_kind = action_execution.get("execution_kind", None)

        if execution_kind is None or execution_kind in excluded_execution_kinds:
            continue

        if category is not None:
            action_category = action_execution.get("name", {}).get("category", None)
            if action_category != category:
                continue

        if target_name is not None:
            action_target_name = (
                action_execution.get("key", {})
                .get("owner", {})
                .get("TargetLabel", {})
                .get("label", {})
                .get("name", None)
            )
            if action_target_name != target_name:
                continue

        return execution_kind

    return None
