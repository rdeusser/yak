# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import tempfile
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_out_single_default_output(yak: Yak) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "output")
        await yak.build("//:a", "--out", output)
        with open(output) as readable:
            assert readable.read() == "a\n"


@yak_test()
async def test_out_overwrite(yak: Yak) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "output")
        await yak.build("//:a", "--out", output)
        await yak.build("//:a", "--out", output)


@yak_test()
async def test_out_parent_not_exist(yak: Yak) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "notexist", "output")
        await yak.build("//:a", "--out", output)
        with open(output) as readable:
            assert readable.read() == "a\n"


@yak_test()
async def test_out_single_default_output_to_dir(yak: Yak) -> None:
    with tempfile.TemporaryDirectory() as out:
        await yak.build("//:a", "--out", out)
        with open(Path(out) / "a.txt") as readable:
            assert readable.read() == "a\n"


@yak_test()
async def test_out_no_outputs(yak: Yak) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            yak.build("//:none", "--out", out.name),
            stderr_regex="produced zero default outputs",
        )


@yak_test()
async def test_out_multiple_outputs(yak: Yak) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            yak.build("//:ab", "--out", out.name),
            stderr_regex="produced 2 outputs",
        )


@yak_test()
async def test_out_multiple_targets(yak: Yak) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            yak.build("//:a", "//:b", "--out", out.name),
            stderr_regex="command built multiple top-level targets",
        )


@yak_test()
async def test_out_directory(yak: Yak) -> None:
    with tempfile.TemporaryDirectory() as out:
        await yak.build("//:dir", "--out", out)
        assert (Path(out) / "b.txt").exists()
        assert (Path(out) / "nested_dir" / "a.txt").exists()


@yak_test()
async def test_out_stdout_multiple(yak: Yak) -> None:
    result = await yak.build("//:a", "//:b", "--out", "-")

    # The e2e test runner adds a `--build-report` flag in order to be able
    # to parse out failures. In normal usage of `--out -` there wouldn't be this
    # extra line of JSON on the stdout, we'd _just_ get the requested outputs.
    a, b, build_report, trailing = result.stdout.split("\n")
    assert (a, b) == ("a", "b") or (a, b) == ("b", "a")
    assert build_report.startswith("{")
    assert trailing == ""


@yak_test()
async def test_out_stdout_none(yak: Yak) -> None:
    await yak.build("--out", "-")


@yak_test()
async def test_out_stdout_directory(yak: Yak) -> None:
    await expect_failure(
        yak.build("//:dir", "--out", "-"),
        stderr_regex="produces a default output that is a directory, and cannot be sent to stdout",
    )
