# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_clean_stale_bxl(yak: Yak) -> None:
    await yak.bxl("//clean_stale/build.bxl:build_test")

    art_files = [path.name for path in (yak.cwd / "yak-out/v2/art").glob("**/*")]
    assert "out.json" in art_files

    # Check that artifacts written to art by bxl are not deleted
    await yak.clean("--stale")
    art_files = [path.name for path in (yak.cwd / "yak-out/v2/art").glob("**/*")]
    assert "out.json" in art_files

    # Force clean of tracked artifacts, check that art and bxl are both deleted
    await yak.kill()
    await yak.clean("--stale=0s")

    art_files = [path.name for path in (yak.cwd / "yak-out/v2/art").glob("**/*")]
    assert "out.json" not in art_files

    art_bxl_files = [
        path.name for path in (yak.cwd / "yak-out/v2/art-bxl").glob("**/*")
    ]
    assert "foo_out" not in art_bxl_files
