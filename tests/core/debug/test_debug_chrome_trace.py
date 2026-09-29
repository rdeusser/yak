# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os.path
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_chrome_trace(yak: Yak, tmp_path: Path) -> None:
    # Just check it at least runs.
    await yak.build("//...")
    await yak.debug("chrome-trace", "--trace-path", str(tmp_path / "trace.json"))


@yak_test()
async def test_chrome_trace_no_repo(yak: Yak, tmp_path: Path) -> None:
    # Check that it runs from a path that is not in the repo.
    await yak.build("//...")
    log_path = (await yak.log("last")).stdout.strip()
    await yak.debug(
        "chrome-trace",
        "--trace-path",
        str(tmp_path / "trace.json"),
        "--path",
        log_path,
        rel_cwd=Path(os.path.relpath("/", yak.cwd)),
    )
