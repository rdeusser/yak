# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import random
import string
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test(write_invocation_record=True)
async def test_bxl_exec_platform_dynamic_output(yak: Yak) -> None:
    result = await yak.bxl(
        "//executor_fallback_tests/dynamic.bxl:test_dynamic_output",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(yak.cwd / Path(output))

    res = await expect_failure(
        yak.bxl(
            "//executor_fallback_tests/dynamic.bxl:test_dynamic_output",
            "-c",
            f"test.cache_buster={random_string()}",
            "--remote-only",
        ),
        stderr_regex="Incompatible executor preferences",
    )

    record = res.invocation_record()
    errors = record["errors"]

    assert len(errors) == 1
    assert errors[0]["category"] == "USER"


@pytest.mark.remote_execution
@yak_test()
async def test_bxl_execution_platforms(yak: Yak) -> None:
    result = await yak.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_platforms",
        "-c",
        f"test.cache_buster={random_string()}",
        "--",
        "--exec_deps",
        "//executor_fallback_tests:remote_only",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(yak.cwd / Path(output))

    await expect_failure(
        yak.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_platforms",
            "-c",
            f"test.cache_buster={random_string()}",
            "--",
            "--exec_deps",
            "//executor_fallback_tests:local_only",
        )
    )

    result = await yak.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_platforms",
        "-c",
        f"test.cache_buster={random_string()}",
        "--",
        "--toolchains",
        "//executor_fallback_tests:remote_only_toolchain",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(yak.cwd / Path(output))

    await expect_failure(
        yak.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_platforms",
            "-c",
            f"test.cache_buster={random_string()}",
            "--",
            "--toolchains",
            "//executor_fallback_tests:local_only_toolchain",
        )
    )

    result = await yak.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_compatible_with",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(yak.cwd / Path(output))

    await expect_failure(
        yak.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_compatible_with",
            "-c",
            f"test.cache_buster={random_string()}",
            "--remote-only",
        )
    )


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))
