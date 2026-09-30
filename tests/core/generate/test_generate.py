# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

MEMBER_BUILD_FILE = """load("@crates//:workspace.bzl", "cargo_workspace_member")

cargo_workspace_member()
"""


@yak_test()
async def test_generate_builds_the_workspace(yak: Yak) -> None:
    await yak.generate()
    for member in ["app", "util"]:
        assert (yak.cwd / member / "YAK").read_text() == MEMBER_BUILD_FILE

    result = await yak.run("//app:app")
    assert (
        result.stdout
        == "util 1.2.3-beta.1 major=1 build_script=yes description=first description origin=hidden\n"
    )

    # The cell reads the manifests, so an edit reaches the next build without
    # another `yak generate`.
    manifest = yak.cwd / "util" / "Cargo.toml"
    manifest.write_text(
        manifest.read_text().replace("first description", "second description")
    )
    result = await yak.run("//app:app")
    assert result.stdout.endswith("description=second description origin=hidden\n")


@yak_test()
async def test_generate_twice_changes_nothing(yak: Yak) -> None:
    await yak.generate()
    config = (yak.cwd / ".yakconfig").read_text()

    result = await yak.generate()
    assert "Wrote the build files of 0 workspace members, and left 2 unchanged" in (
        result.stderr
    )
    assert (yak.cwd / ".yakconfig").read_text() == config


@yak_test()
async def test_generate_keeps_a_different_build_file(yak: Yak) -> None:
    build_file = yak.cwd / "app" / "YAK"
    build_file.write_text("# hand-written\n")

    result = await yak.generate()
    assert "Pass `--force` to replace it" in result.stderr
    assert build_file.read_text() == "# hand-written\n"

    await yak.generate("--force")
    assert build_file.read_text() == MEMBER_BUILD_FILE
