# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import subprocess
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

MEMBER_BUILD_FILE = """load("@crates//:workspace.bzl", "cargo_workspace_member")

cargo_workspace_member()
"""


@yak_test(data_dir="workspace")
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


@yak_test(data_dir="workspace")
async def test_generate_twice_changes_nothing(yak: Yak) -> None:
    await yak.generate()
    config = (yak.cwd / ".yakconfig").read_text()

    result = await yak.generate()
    assert "Wrote the build files of 0 workspace members, and left 2 unchanged" in (
        result.stderr
    )
    assert (yak.cwd / ".yakconfig").read_text() == config


@yak_test(data_dir="workspace")
async def test_generate_keeps_a_different_build_file(yak: Yak) -> None:
    build_file = yak.cwd / "app" / "YAK"
    build_file.write_text("# hand-written\n")

    result = await yak.generate()
    assert "Pass `--force` to replace it" in result.stderr
    assert build_file.read_text() == "# hand-written\n"

    await yak.generate("--force")
    assert build_file.read_text() == MEMBER_BUILD_FILE


@yak_test(data_dir="vendored")
async def test_generate_builds_replaced_sources(yak: Yak) -> None:
    # `.cargo/config.toml` replaces crates.io with `vendor/`, as it would with a
    # mirror of a private registry. The cell takes each package from where
    # Cargo put it.
    await yak.generate()
    result = await yak.run("//app:app")
    assert result.stdout == "hello from vendored\n"


def commit(repository: Path, message: str) -> None:
    for argv in (["add", "."], ["commit", "-q", "-m", message]):
        subprocess.run(
            ["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *argv],
            cwd=repository,
            check=True,
        )


@yak_test(data_dir="git")
async def test_generate_builds_a_git_dependency_at_its_locked_commit(
    yak: Yak,
) -> None:
    # Cargo keeps its checkouts of the repository in a home of the test's own.
    cargo_env = {**os.environ, "CARGO_HOME": str(yak.cwd.parent / "cargo-home")}
    yak.set_env("CARGO_HOME", cargo_env["CARGO_HOME"])

    repository = yak.cwd / "greet"
    subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
    commit(repository, "first")
    manifest = yak.cwd / "app" / "Cargo.toml"
    manifest.write_text(
        manifest.read_text().replace("GREET_REPOSITORY", repository.as_uri())
    )
    subprocess.run(
        ["cargo", "generate-lockfile"], cwd=yak.cwd, env=cargo_env, check=True
    )

    await yak.generate()
    result = await yak.run("//app:app")
    assert result.stdout == "hello from the first commit\n"

    # A new commit keeps the package's version. The cell copies the new sources
    # to another directory, so the build does not reuse the first commit's.
    lib = repository / "src" / "lib.rs"
    lib.write_text(lib.read_text().replace("first", "second"))
    commit(repository, "second")
    subprocess.run(
        ["cargo", "update", "-p", "greet"], cwd=yak.cwd, env=cargo_env, check=True
    )
    result = await yak.run("//app:app")
    assert result.stdout == "hello from the second commit\n"
