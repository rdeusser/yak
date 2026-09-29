# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_yakconfig_works_in_external_cells(yak: Yak) -> None:
    result = await yak.audit(
        "config", "--cell", "test_bundled_cell", "user_section.key"
    )
    assert "key = value" in result.stdout


@yak_test()
async def test_uquery(yak: Yak) -> None:
    result = await yak.uquery("deps(other//:other_alias)")
    assert result.stdout.strip().split() == [
        "test_bundled_cell//dir:test_hidden",
        "test_bundled_cell//dir:test",
        "other//:other_alias",
    ]
    result = await yak.uquery(
        "deps(test_bundled_cell//dir:test)", rel_cwd=Path("other")
    )
    assert result.stdout.strip().split() == [
        "test_bundled_cell//dir:test_hidden",
        "test_bundled_cell//dir:test",
    ]


@yak_test()
async def test_build_local(yak: Yak) -> None:
    result = await yak.build_without_report(
        "--show-full-simple-output", "--local-only", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar", "foobar2"])


@pytest.mark.remote_execution
@yak_test()
async def test_build_remote(yak: Yak) -> None:
    result = await yak.build_without_report(
        "--show-full-simple-output", "--remote-only", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar", "foobar2"])


@yak_test()
async def test_materialize_source_directly(yak: Yak) -> None:
    result = await yak.build_without_report(
        "--show-full-simple-output", "test_bundled_cell//dir:exported"
    )
    p = Path(result.stdout.strip())
    assert f"external_cells{os.path.sep}bundled" in str(p)
    assert str(p).endswith("src.txt")
    assert p.read_text().strip() == "foobar"


@yak_test()
async def test_expand_external_cell(yak: Yak) -> None:
    await yak.expand_external_cell("test_bundled_cell")
    assert (yak.cwd / "test_bundled_cell" / ".yakconfig").exists()

    # Remove the external cell declaration
    (yak.cwd / ".yakconfig_no_external").replace(yak.cwd / ".yakconfig")
    (yak.cwd / "test_bundled_cell" / "dir" / "src.txt").write_text("foobar3\n")

    result = await yak.build_without_report(
        "--show-full-simple-output", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar3", "foobar2"])
