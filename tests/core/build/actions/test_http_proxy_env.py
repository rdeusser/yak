# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


from __future__ import annotations

import asyncio
import hashlib
import json
import socket
import uuid
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import dataclass

import pytest
from aiohttp import web
from e2e_util.api.buck import Buck
from e2e_util.asserts import expect_failure
from e2e_util.buck_workspace import buck_test


PROXY_ENV_VARS = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "NO_PROXY",
    "no_proxy",
]


def make_payload() -> bytes:
    return f"download contents {uuid.uuid4()}\n".encode()


@dataclass
class Server:
    url: str
    requests: list[tuple[str, str]]


@asynccontextmanager
async def serve(
    payload: bytes,
    *,
    host: str = "127.0.0.1",
    redirects: dict[str, str] | None = None,
) -> AsyncIterator[Server]:
    requests: list[tuple[str, str]] = []

    async def handle(request: web.Request) -> web.Response:
        requests.append((request.method, request.raw_path))
        if redirects and request.raw_path in redirects:
            return web.Response(
                status=302, headers={"Location": redirects[request.raw_path]}
            )
        return web.Response(body=payload)

    app = web.Application()
    app.router.add_route("*", "/{path:.*}", handle)
    runner = web.AppRunner(app, access_log=None)
    await runner.setup()
    with socket.socket() as sock:
        try:
            sock.bind((host, 0))
            port = sock.getsockname()[1]
            site = web.SockSite(runner, sock)
            await site.start()
            yield Server(f"http://{host}:{port}", requests)
        finally:
            await runner.cleanup()


def configure(buck: Buck, digest: str = "SHA256") -> None:
    for name in PROXY_ENV_VARS:
        assert buck.get_env_var(name) is None, (
            f"{name} leaked into the test environment"
        )
    (buck.cwd / ".yakconfig.local").write_text(
        f"[yak]\ndigest_algorithms = {digest}\n"
    )


def build_args(
    payload: bytes,
    origin: Server,
    *,
    size: bool = False,
) -> list[str]:
    args = [
        "--local-only",
        "--no-remote-cache",
        "-c",
        f"test.url={origin.url}/download",
        "-c",
        f"test.sha256={hashlib.sha256(payload).hexdigest()}",
    ]
    if size:
        args.extend(["-c", f"test.size_bytes={len(payload)}"])
    return args


async def build_download(
    buck: Buck,
    payload: bytes,
    origin: Server,
    env: dict[str, str],
    *,
    size: bool = False,
) -> None:
    result = await asyncio.wait_for(
        buck.build(
            "//:download",
            *build_args(payload, origin, size=size),
            env=env,
        ),
        timeout=90,
    )
    output = result.get_build_report().output_for_target("root//:download")
    assert output.read_bytes() == payload


async def daemon_pid(buck: Buck, env: dict[str, str]) -> int:
    result = await asyncio.wait_for(buck.status(env=env), timeout=30)
    return json.loads(result.stdout)["process_info"]["pid"]


class TestHttpProxyEnv:
    @pytest.fixture(autouse=True)
    def clear_proxy_environment(self, monkeypatch: pytest.MonkeyPatch) -> None:
        for name in PROXY_ENV_VARS:
            monkeypatch.delenv(name, raising=False)

    @pytest.mark.parametrize(
        "digest,size,methods",
        [
            ("SHA1", False, ["GET"]),
            ("SHA256", False, ["HEAD", "GET"]),
            ("SHA256", True, ["GET"]),
        ],
        ids=["immediate", "head-and-deferred-get", "sized-deferred-get"],
    )
    @buck_test(skip_for_os=["windows"])
    async def test_proxy_download_paths(
        self, buck: Buck, digest: str, size: bool, methods: list[str]
    ) -> None:
        configure(buck, digest=digest)
        payload = make_payload()
        async with serve(payload) as origin, serve(payload) as proxy:
            await build_download(
                buck,
                payload,
                origin,
                {"HTTP_PROXY": proxy.url},
                size=size,
            )
            assert origin.requests == []
            assert proxy.requests == [
                (method, f"{origin.url}/download") for method in methods
            ]

    @pytest.mark.parametrize(
        "values,proxied",
        [
            ({}, False),
            ({"http_proxy": "proxy"}, True),
            (
                {"HTTP_PROXY": "proxy", "http_proxy": "origin"},
                True,
            ),
            ({"HTTP_PROXY": "proxy", "NO_PROXY": "127.0.0.1"}, False),
            ({"HTTP_PROXY": "proxy", "no_proxy": "127.0.0.1"}, False),
            ({"NO_PROXY": "127.0.0.1"}, False),
            ({"SDK_PROXY": "proxy", "Http_Proxy": "proxy"}, False),
            ({"ALL_PROXY": "proxy", "all_proxy": "proxy"}, False),
        ],
        ids=[
            "all-unset",
            "lowercase",
            "uppercase-precedence",
            "no-proxy-exclusion",
            "lowercase-no-proxy-exclusion",
            "no-proxy-alone",
            "nonstandard-variables-ignored",
            "all-proxy-ignored",
        ],
    )
    @buck_test(skip_for_os=["windows"])
    async def test_proxy_environment_semantics(
        self, buck: Buck, values: dict[str, str], proxied: bool
    ) -> None:
        configure(buck)
        payload = make_payload()
        async with serve(payload) as origin, serve(payload) as proxy:
            substitutions = {"origin": origin.url, "proxy": proxy.url}
            env = {
                name: substitutions.get(value, value) for name, value in values.items()
            }
            await build_download(buck, payload, origin, env)
            if proxied:
                assert origin.requests == []
                assert proxy.requests == [
                    ("HEAD", f"{origin.url}/download"),
                    ("GET", f"{origin.url}/download"),
                ]
            else:
                assert proxy.requests == []
                assert origin.requests == [
                    ("HEAD", "/download"),
                    ("GET", "/download"),
                ]

    @pytest.mark.parametrize(
        "values",
        [
            {"HTTP_PROXY": ""},
            {"http_proxy": ""},
            {"HTTPS_PROXY": ""},
            {"https_proxy": ""},
            {"HTTP_PROXY": "not a valid proxy"},
        ],
        ids=[
            "empty-http",
            "empty-http-lowercase",
            "empty-https",
            "empty-https-lowercase",
            "malformed",
        ],
    )
    @buck_test(skip_for_os=["windows"])
    async def test_invalid_proxy_values_are_rejected(
        self, buck: Buck, values: dict[str, str]
    ) -> None:
        configure(buck)
        payload = make_payload()
        # The daemon fails to start with an invalid proxy value. The client
        # sometimes notices only when its startup timeout runs out, so this
        # test shortens the 120 seconds that the harness sets.
        startup_timeout = {
            "YAKD_STARTUP_TIMEOUT": "30",
            "YAKD_STARTUP_INIT_TIMEOUT": "30",
        }
        async with serve(payload) as origin:
            await asyncio.wait_for(
                expect_failure(
                    buck.build(
                        "//:download",
                        *build_args(payload, origin),
                        env={**values, **startup_timeout},
                    ),
                    stderr_regex="Invalid (HTTP|HTTPS)_PROXY uri",
                ),
                timeout=90,
            )
            assert origin.requests == []

    @buck_test(skip_for_os=["windows"])
    async def test_proxy_environment_changes_require_manual_restart(
        self, buck: Buck
    ) -> None:
        configure(buck)
        payload = make_payload()
        second_payload = make_payload()
        async with (
            serve(payload) as origin,
            serve(payload) as first_proxy,
            serve(second_payload) as second_origin,
            serve(second_payload) as second_proxy,
        ):
            args = build_args(payload, origin)
            first_env = {"HTTP_PROXY": first_proxy.url}
            first_result = await asyncio.wait_for(
                buck.build(
                    "//:download", *args, "--materializations=None", env=first_env
                ),
                timeout=90,
            )
            output = first_result.get_build_report().output_for_target(
                "root//:download"
            )
            assert not output.exists()
            assert first_proxy.requests == [("HEAD", f"{origin.url}/download")]
            first_pid = await daemon_pid(buck, first_env)

            second_env = {"HTTP_PROXY": second_proxy.url}
            await asyncio.wait_for(
                buck.build("//:download", *args, env=second_env),
                timeout=90,
            )
            assert await daemon_pid(buck, second_env) == first_pid
            assert output.read_bytes() == payload
            assert first_proxy.requests == [
                ("HEAD", f"{origin.url}/download"),
                ("GET", f"{origin.url}/download"),
            ]
            assert second_proxy.requests == []

            await asyncio.wait_for(buck.kill(), timeout=30)
            second_result = await asyncio.wait_for(
                buck.build(
                    "//:download",
                    *build_args(second_payload, second_origin),
                    env=second_env,
                ),
                timeout=90,
            )
            assert await daemon_pid(buck, second_env) != first_pid
            output = second_result.get_build_report().output_for_target(
                "root//:download"
            )
            assert output.read_bytes() == second_payload
            assert first_proxy.requests == [
                ("HEAD", f"{origin.url}/download"),
                ("GET", f"{origin.url}/download"),
            ]
            assert second_proxy.requests == [
                ("HEAD", f"{second_origin.url}/download"),
                ("GET", f"{second_origin.url}/download"),
            ]
            assert origin.requests == []
            assert second_origin.requests == []
