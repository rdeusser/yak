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
import uuid

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.http_server import StaticHttpServer


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


def serve(
    payload: bytes | dict[str, bytes],
    *,
    host: str = "127.0.0.1",
    redirects: dict[str, str] | None = None,
) -> StaticHttpServer:
    """
    A server answering every request with `payload`, or routing by what was asked for when given a
    mapping.
    """
    return StaticHttpServer(payload, host=host, redirects=redirects)


def configure(yak: Yak, digest: str = "SHA256") -> None:
    for name in PROXY_ENV_VARS:
        assert yak.get_env_var(name) is None, (
            f"{name} leaked into the test environment"
        )
    (yak.cwd / ".yakconfig.local").write_text(
        f"[yak]\ndigest_algorithms = {digest}\n"
    )


def build_args(
    payload: bytes,
    origin: StaticHttpServer,
    *,
    size: bool = False,
) -> list[str]:
    args = [
        "--local-only",
        "--no-remote-cache",
        "-c",
        f"test.url={origin.url('/download')}",
        "-c",
        f"test.sha256={hashlib.sha256(payload).hexdigest()}",
    ]
    if size:
        args.extend(["-c", f"test.size_bytes={len(payload)}"])
    return args


async def build_download(
    yak: Yak,
    payload: bytes,
    origin: StaticHttpServer,
    env: dict[str, str],
    *,
    size: bool = False,
) -> None:
    result = await asyncio.wait_for(
        yak.build(
            "//:download",
            *build_args(payload, origin, size=size),
            env=env,
        ),
        timeout=90,
    )
    output = result.get_build_report().output_for_target("root//:download")
    assert output.read_bytes() == payload


async def daemon_pid(yak: Yak, env: dict[str, str]) -> int:
    result = await asyncio.wait_for(yak.status(env=env), timeout=30)
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
        ids=["get-only", "head-then-get", "sized-get-only"],
    )
    @yak_test(skip_for_os=["windows"])
    async def test_proxy_download_paths(
        self, yak: Yak, digest: str, size: bool, methods: list[str]
    ) -> None:
        configure(yak, digest=digest)
        payload = make_payload()
        async with serve(payload) as origin, serve(payload) as proxy:
            await build_download(
                yak,
                payload,
                origin,
                {"HTTP_PROXY": proxy.url()},
                size=size,
            )
            assert origin.requests == []
            assert proxy.requests == [
                (method, origin.url("/download")) for method in methods
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
    @yak_test(skip_for_os=["windows"])
    async def test_proxy_environment_semantics(
        self, yak: Yak, values: dict[str, str], proxied: bool
    ) -> None:
        configure(yak)
        payload = make_payload()
        async with serve(payload) as origin, serve(payload) as proxy:
            substitutions = {"origin": origin.url(), "proxy": proxy.url()}
            env = {
                name: substitutions.get(value, value) for name, value in values.items()
            }
            await build_download(yak, payload, origin, env)
            if proxied:
                assert origin.requests == []
                assert proxy.requests == [
                    ("HEAD", origin.url("/download")),
                    ("GET", origin.url("/download")),
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
    @yak_test(skip_for_os=["windows"])
    async def test_invalid_proxy_values_are_rejected(
        self, yak: Yak, values: dict[str, str]
    ) -> None:
        configure(yak)
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
                    yak.build(
                        "//:download",
                        *build_args(payload, origin),
                        env={**values, **startup_timeout},
                    ),
                    stderr_regex="Invalid (HTTP|HTTPS)_PROXY uri",
                ),
                timeout=90,
            )
            assert origin.requests == []

    @yak_test(skip_for_os=["windows"])
    async def test_proxy_environment_changes_require_manual_restart(
        self, yak: Yak
    ) -> None:
        configure(yak)
        payloads = [make_payload() for _ in range(3)]
        async with (
            serve(payloads[0]) as origin,
            serve(payloads[1]) as second_origin,
            serve(payloads[2]) as third_origin,
        ):
            origins = [origin, second_origin, third_origin]
            # Either proxy can be asked for any of the three, and answers as that origin would.
            routes = {
                server.url("/download"): payload
                for server, payload in zip(origins, payloads)
            }
            async with serve(routes) as first_proxy, serve(routes) as second_proxy:
                first_env = {"HTTP_PROXY": first_proxy.url()}
                await build_download(yak, payloads[0], origin, first_env)
                first_pid = await daemon_pid(yak, first_env)
                assert first_proxy.requests == [
                    ("HEAD", origin.url("/download")),
                    ("GET", origin.url("/download")),
                ]

                # A download in the same daemon, asked for with a different proxy in the
                # environment, still goes through the proxy the daemon started with.
                second_env = {"HTTP_PROXY": second_proxy.url()}
                await build_download(yak, payloads[1], second_origin, second_env)
                assert await daemon_pid(yak, second_env) == first_pid
                assert second_proxy.requests == []
                assert first_proxy.requests[-2:] == [
                    ("HEAD", second_origin.url("/download")),
                    ("GET", second_origin.url("/download")),
                ]

                # Only a restart picks the new one up.
                await asyncio.wait_for(yak.kill(), timeout=30)
                await build_download(yak, payloads[2], third_origin, second_env)
                assert await daemon_pid(yak, second_env) != first_pid
                assert second_proxy.requests == [
                    ("HEAD", third_origin.url("/download")),
                    ("GET", third_origin.url("/download")),
                ]
                assert [server.requests for server in origins] == [[], [], []]
