# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import re
import subprocess
import sys
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException
from e2e_util.yak_workspace import yak_test

WORKSPACE_BUILD_FILE = """load("@crates//:workspace.bzl", "cargo_workspace")

cargo_workspace()
"""


@yak_test(data_dir="workspace")
async def test_generate_builds_the_workspace(yak: Yak) -> None:
    await yak.generate()
    assert (yak.cwd / "YAK").read_text() == WORKSPACE_BUILD_FILE
    for member in ["app", "util"]:
        assert not (yak.cwd / member / "YAK").exists()

    result = await yak.run("//:app")
    assert (
        result.stdout
        == "util 1.2.3-beta.1 major=1 build_script=yes description=first description origin=hidden banner=shared\n"
    )

    # The cell reads the manifests, so an edit reaches the next build without
    # another `yak generate`.
    manifest = yak.cwd / "util" / "Cargo.toml"
    manifest.write_text(
        manifest.read_text().replace("first description", "second description")
    )
    result = await yak.run("//:app")
    assert result.stdout.endswith(
        "description=second description origin=hidden banner=shared\n"
    )


@yak_test(data_dir="workspace")
async def test_generate_twice_changes_nothing(yak: Yak) -> None:
    await yak.generate()
    config = (yak.cwd / ".yakconfig").read_text()

    result = await yak.generate()
    assert "`YAK` is up to date" in result.stderr
    assert (yak.cwd / ".yakconfig").read_text() == config


@yak_test(data_dir="workspace")
async def test_generate_keeps_a_different_build_file(yak: Yak) -> None:
    build_file = yak.cwd / "YAK"
    build_file.write_text("# hand-written\n")

    result = await yak.generate()
    assert "Pass `--force` to replace it" in result.stderr
    assert build_file.read_text() == "# hand-written\n"

    await yak.generate("--force")
    assert build_file.read_text() == WORKSPACE_BUILD_FILE


@yak_test(data_dir="workspace")
async def test_generate_rejects_a_build_file_in_a_member(yak: Yak) -> None:
    await yak.generate()
    # The file would make `app` a package of its own, outside the workspace's package.
    (yak.cwd / "app" / "YAK").write_text("# hand-written\n")
    with pytest.raises(YakException) as e:
        await yak.generate("--force")
    assert "`app/YAK` would make workspace members packages of their own" in str(
        e.value
    )


@yak_test(data_dir="vendored")
async def test_generate_builds_replaced_sources(yak: Yak) -> None:
    # `.cargo/config.toml` replaces crates.io with `vendor/`, as it would with a
    # mirror of a private registry. The cell takes each package from where
    # Cargo put it.
    await yak.generate()
    result = await yak.run("//:app")
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
    result = await yak.run("//:app")
    assert result.stdout == "hello from the first commit\n"

    # A new commit keeps the package's version. The cell copies the new sources
    # to another directory, so the build does not reuse the first commit's.
    lib = repository / "src" / "lib.rs"
    lib.write_text(lib.read_text().replace("first", "second"))
    commit(repository, "second")
    subprocess.run(
        ["cargo", "update", "-p", "greet"], cwd=yak.cwd, env=cargo_env, check=True
    )
    result = await yak.run("//:app")
    assert result.stdout == "hello from the second commit\n"


def build_host_library(directory: Path) -> None:
    """Builds `libhostanswer.a`, whose `host_answer` returns 42, in `directory`."""
    source = directory / "answer.c"
    source.write_text("int host_answer(void) { return 42; }\n")
    subprocess.run(
        ["cc", "-c", str(source), "-o", str(directory / "answer.o")], check=True
    )
    # Apple's `ar` writes the member alignment that the macOS linker requires.
    ar = ["xcrun", "ar"] if sys.platform == "darwin" else ["ar"]
    subprocess.run(
        [*ar, "rcs", str(directory / "libhostanswer.a"), str(directory / "answer.o")],
        check=True,
    )


@yak_test(data_dir="hostlib")
async def test_generate_links_a_host_library_of_a_build_script(yak: Yak) -> None:
    # The build script of `host` names a library in a directory outside the
    # project, as one that finds a system library through pkg-config does. The
    # link of `app` needs the directory, which rustc does not record in the
    # library of `host`.
    host_lib_dir = yak.cwd.parent / "host-lib"
    host_lib_dir.mkdir()
    build_host_library(host_lib_dir)
    (yak.cwd / "host" / "host-lib-dir.txt").write_text(str(host_lib_dir))

    await yak.generate()
    result = await yak.run("//:app")
    assert result.stdout == "answer=42\n"


@yak_test(data_dir="sharedlib")
async def test_generate_runs_a_binary_that_loads_a_build_script_shared_library(
    yak: Yak,
) -> None:
    # The build script of `seven` compiles `libseven` into its `OUT_DIR` and
    # links it, as Cargo lets a build script do. Cargo puts the directory on
    # the dynamic library path of the programs it runs, and so does yak.
    await yak.generate()
    result = await yak.run("//:app")
    assert result.stdout == "seven=7\n"
    await yak.test("//:seven-unittest")


@yak_test(data_dir="sharedlib")
async def test_generate_builds_a_build_script_only_for_its_run(yak: Yak) -> None:
    # The build script of `seven` has an alias per Cargo platform, which its run
    # target selects from. `//...` builds the library, whose build script run
    # builds the script for the execution platform, and skips the script and
    # the aliases.
    await yak.generate()
    result = await yak.build("//...", "--show-output")
    built = [
        line.split()[0]
        for line in result.stdout.splitlines()
        if line.startswith("root//")
    ]
    assert "root//:seven" in built
    assert [target for target in built if "build-script-build" in target] == []
    # They build only for an execution platform, so the build does not list
    # them as skipped.
    assert "incompatible" not in result.stderr, result.stderr
    result = await yak.test("//...")
    assert "incompatible" not in result.stderr, result.stderr


@yak_test(data_dir="cargotest")
async def test_generate_runs_tests_as_cargo_test_does(yak: Yak) -> None:
    # The tests find the package's examples from their own path, and the
    # integration test runs the binary through `CARGO_BIN_EXE_probe`.
    await yak.generate()
    targets = (await yak.targets("//:")).stdout.split()
    assert "root//:probe-example-plugin" in targets
    assert "root//:probe-probe-unittest" in targets
    # `extra` needs a feature that is off, so Cargo skips it.
    assert "root//:extra" not in targets
    result = await yak.test("//...")
    summary = re.sub("\x1b\\[[0-9;]*m", "", result.stderr)
    assert "Pass 3. Fail 0." in summary, result.stderr
