# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import platform
from typing import Iterable

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_clean(yak: Yak) -> None:
    build_result = await yak.build("root//:trivial_build")
    build_report = build_result.get_build_report()
    build_report_outputs = [
        str(output)
        for output in build_report.outputs_for_target("root//:trivial_build")
    ]

    clean_result = await yak.clean()
    clean_paths = tuple(filter(None, clean_result.stderr.split("\n")))

    for output in build_report_outputs:
        assert output.startswith(clean_paths)

    _assert_all_paths_do_not_exist(build_report_outputs)


@yak_test()
async def test_clean_dry_run(yak: Yak) -> None:
    build_result = await yak.build("root//:trivial_build", "--show-output")
    build_report = build_result.get_build_report()
    build_report_outputs = [
        str(output)
        for output in build_report.outputs_for_target("root//:trivial_build")
    ]

    dry_clean_result = await yak.clean("--dry-run")

    dry_clean_paths = set(
        filter(
            is_yak_path,
            dry_clean_result.stderr.split("\n"),
        )
    )
    _assert_all_paths_exist(dry_clean_paths)

    dry_clean_paths = tuple(i for i in dry_clean_paths)
    for output in build_report_outputs:
        assert output.startswith(dry_clean_paths)

    _assert_all_paths_exist(build_report_outputs)

    # Run clean without dry-run and make sure all files are removed now
    clean_result = await yak.clean()
    clean_paths = set(
        filter(
            is_yak_path,
            clean_result.stderr.split("\n"),
        )
    )
    # dry_clean_paths and clean_paths should be the same
    for clean_path in clean_paths:
        assert clean_path in dry_clean_paths
    for dry_clean_path in dry_clean_paths:
        assert dry_clean_path in clean_paths

    _assert_all_paths_do_not_exist(clean_paths)


def is_yak_path(x: str) -> bool:
    if platform.system() == "Windows":
        return "\\.yak\\yakd\\" in x or "\\yak-out\\" in x
    else:
        return "/.yak/yakd/" in x or "/yak-out/" in x


def _assert_all_paths_exist(paths: Iterable[str]) -> None:
    for path in paths:
        assert os.path.exists(path) is True


def _assert_all_paths_do_not_exist(paths: Iterable[str]) -> None:
    for path in paths:
        if os.path.exists(f"{path}/yakd.lifecycle"):
            # Clean keeps lifecycle file in daemon dir.
            assert ["yakd.lifecycle"] == os.listdir(path)
        elif path.endswith("yak-out/v2/log") or (
            platform.system() == "Windows" and path.endswith("yak-out\\v2\\log")
        ):
            # Log dir should contain one entry, for the clean command itself.
            assert len(os.listdir(path)) == 1
        else:
            assert os.path.exists(path) is False


@yak_test()
async def test_isolation_dir_reserved_prefix_rejected(yak: Yak) -> None:
    yak.set_isolation_prefix("._yak_anything")
    try:
        await expect_failure(
            yak.build("root//:trivial_build"),
            stderr_regex="reserved for yak",
        )
    finally:
        # The fixture teardown runs `yak clean`, which would itself trip the
        # reserved-name rejection with the prefix still set.
        yak.set_isolation_prefix("v2")


@yak_test()
async def test_clean_background(yak: Yak) -> None:
    """Test that yak clean --background moves yak-out to trash and deletes it."""
    build_result = await yak.build("root//:trivial_build")
    build_report = build_result.get_build_report()
    build_report_outputs = [
        str(output)
        for output in build_report.outputs_for_target("root//:trivial_build")
    ]

    # Run clean with --background flag
    clean_result = await yak.clean("--background")

    # Check that the output contains the expected messages
    assert "yak-out moved to trash. Now cleaning up..." in clean_result.stderr
    assert "Tip: Use Ctrl-Z to put this in the background" in clean_result.stderr
    assert (
        "You can run other yak commands while this completes." in clean_result.stderr
    )

    # Verify all build outputs are eventually deleted
    _assert_all_paths_do_not_exist(build_report_outputs)
