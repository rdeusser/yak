# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test(data_dir="include_external")
async def test_include_external_file(yak: Yak) -> None:
    # Note that the repo is inside a tempdir
    (yak.cwd.parent / "extra").write_text("[abc]\ndef=x", encoding="utf-8")
    await expect_failure(
        yak.audit_config("--cell", "root"),
        stderr_regex="Improperly include directive path",
    )


@yak_test(data_dir="empty", skip_for_os=["windows"])
async def test_external_symlink_resolution(yak: Yak, tmp_path: Path) -> None:
    base = tmp_path / "base"
    (base / "b" / "bb").mkdir(parents=True)
    (base / "a").mkdir()
    (base / "a" / "aa").symlink_to("../b/bb")
    (base / "b" / "included").write_text("[sec]\nval = physical", encoding="utf-8")
    (base / "a" / "included").write_text("[sec]\nval = logical", encoding="utf-8")

    (base / "b" / "bb" / "config").write_text("<file:../included>", encoding="utf-8")

    config_via_symlink = base / "a" / "aa" / "config"

    res = await yak.audit_config(
        "--cell", "root", "--config-file", str(config_via_symlink)
    )
    assert "val = physical" in res.stdout


@yak_test(data_dir="empty")
async def test_changing_external_include(yak: Yak) -> None:
    extra = yak.cwd.parent / "extra"
    extra.write_text("[abc]\n  def = 1", encoding="utf-8")

    # Start the daemon and build once
    await yak.audit_config(
        "--all-cells", env={"YAK_TEST_EXTRA_EXTERNAL_CONFIG": str(extra)}
    )

    # Change the file and build again
    extra.write_text("[abc]\n    def = 2", encoding="utf-8")

    res = await yak.audit_config("--cell", "root", "abc.def")
    assert "[abc]\n    def = 2" in res.stdout
    res = await yak.audit_config("--cell", "cell", "abc.def")
    assert "[abc]\n    def = 2" in res.stdout


@yak_test(data_dir="include_through_symlink")
async def test_external_symlink_source_file(yak: Yak) -> None:
    external_dir = yak.cwd.parent / "extra"
    external_dir.mkdir()
    (yak.cwd / "repo_dir").symlink_to(external_dir)

    await yak.audit_config("--cell", "root", "abc.def")
