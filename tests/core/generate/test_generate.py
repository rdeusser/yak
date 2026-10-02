# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test

WORKSPACE_BUILD_FILE = """load("@crates//:workspace.bzl", "cargo_workspace")

cargo_workspace()
"""

MEMBER_BUILD_FILE = """load("@crates//:workspace.bzl", "cargo_package")

cargo_package()
"""


@yak_test(data_dir="workspace")
async def test_generate_builds_the_workspace(yak: Yak) -> None:
    await yak.generate()
    assert (yak.cwd / "YAK").read_text() == WORKSPACE_BUILD_FILE
    for member in ["app", "util"]:
        assert (yak.cwd / member / "YAK").read_text() == MEMBER_BUILD_FILE

    result = await yak.run("//app")
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
    result = await yak.run("//app")
    assert result.stdout.endswith(
        "description=second description origin=hidden banner=shared\n"
    )

    # The cell finds the files that the crate includes, so a new include of a
    # file outside the crate's directory builds without a declaration.
    lib = yak.cwd / "util" / "src" / "lib.rs"
    lib.write_text(lib.read_text().replace("shared/banner.txt", "shared/footer.txt"))
    result = await yak.run("//app")
    assert result.stdout.endswith("origin=hidden banner=footer\n")


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


@yak_test(data_dir="vendored")
async def test_generate_builds_replaced_sources(yak: Yak) -> None:
    # `.cargo/config.toml` replaces crates.io with `vendor/`, as it would with a
    # mirror of a private registry. The cell takes each package from where
    # Cargo put it.
    await yak.generate()
    result = await yak.run("//app")
    assert result.stdout == "hello from vendored\n"


@yak_test(data_dir="vendored")
async def test_generate_applies_the_rustflags_of_the_cargo_configuration(
    yak: Yak,
) -> None:
    (yak.cwd / "app" / "src" / "main.rs").write_text(
        'fn main() {\n    println!("{} flag={}", greet::greeting(), cfg!(yak_flag));\n}\n'
    )
    config = yak.cwd / ".cargo" / "config.toml"
    vendoring = config.read_text()
    config.write_text(vendoring + '\n[build]\nrustflags = ["--cfg", "yak_flag"]\n')
    await yak.generate()
    result = await yak.run("//app")
    assert result.stdout == "hello from vendored flag=true\n"

    # A `target` setting that matches the platform replaces `build.rustflags`,
    # and the cell reads the configuration through DICE, so the edit reaches
    # the next build.
    config.write_text(
        vendoring
        + '\n[build]\nrustflags = ["--cfg", "yak_flag"]\n'
        + "\n[target.'cfg(all())']\nrustflags = []\n"
    )
    result = await yak.run("//app")
    assert result.stdout == "hello from vendored flag=false\n"



@yak_test(data_dir="vendored")
async def test_generate_applies_the_dev_profile(yak: Yak) -> None:
    (yak.cwd / "app" / "src" / "main.rs").write_text(
        """fn main() {
    println!("{} debug_assertions={}", greet::greeting(), cfg!(debug_assertions));
    if std::env::args().nth(1).as_deref() == Some("panic") {
        let caught = std::panic::catch_unwind(|| panic!("boom")).is_err();
        println!("caught={caught}");
    }
}

#[test]
fn tests_unwind() {
    greet::greeting();
    assert!(std::panic::catch_unwind(|| panic!("boom")).is_err());
}
"""
    )
    manifest = yak.cwd / "Cargo.toml"
    workspace = manifest.read_text()
    manifest.write_text(
        workspace + '\n[profile.dev]\ndebug-assertions = false\npanic = "abort"\n'
    )
    await yak.generate()
    # `greet` calls through the `C-unwind` ABI, so the binary links only if
    # `greet` compiled with `-Cpanic=abort` too.
    result = await yak.run("//app")
    assert result.stdout == "hello from vendored debug_assertions=false\n"
    # With `panic = "abort"`, the panic aborts the binary before `catch_unwind`
    # returns.
    await expect_failure(yak.run("//app", "--", "panic"), stderr_regex="boom")
    # As with Cargo, tests unwind, and they link a copy of `greet` that unwinds.
    await yak.test("//app")

    # The cell reads the workspace's manifest through DICE, so the edit reaches
    # the next build.
    manifest.write_text(workspace)
    result = await yak.run("//app", "--", "panic")
    assert (
        result.stdout == "hello from vendored debug_assertions=true\ncaught=true\n"
    )

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
    result = await yak.run("//app")
    assert result.stdout == "hello from the first commit\n"

    # A new commit keeps the package's version. The cell copies the new sources
    # to another directory, so the build does not reuse the first commit's.
    lib = repository / "src" / "lib.rs"
    lib.write_text(lib.read_text().replace("first", "second"))
    commit(repository, "second")
    subprocess.run(
        ["cargo", "update", "-p", "greet"], cwd=yak.cwd, env=cargo_env, check=True
    )
    result = await yak.run("//app")
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
    result = await yak.run("//app")
    assert result.stdout == "answer=42\n"


@yak_test(data_dir="buildscriptinclude")
async def test_generate_runs_a_build_script_in_the_workspace_layout(yak: Yak) -> None:
    # The build script of `crates/gen` reads `../../proto/api.txt`, which
    # `include` of its build file declares, from the package's directory.
    await yak.generate()
    result = await yak.run("//crates/gen")
    # `file!()` names the file relative to the workspace's directory, as with
    # Cargo.
    assert result.stdout == "api=v1 file=crates/gen/src/main.rs\n"

    # The file is an input of the script's run, so an edit reaches the next
    # build.
    (yak.cwd / "proto" / "api.txt").write_text("v2\n")
    result = await yak.run("//crates/gen")
    assert result.stdout == "api=v2 file=crates/gen/src/main.rs\n"


@yak_test(data_dir="includecell")
async def test_generate_includes_the_files_of_another_cell(yak: Yak) -> None:
    # `include` of `crates/gen` names the `source_listing` of the `assets`
    # cell, which lists the files of its package `sub` too.
    await yak.generate()
    config = yak.cwd / ".yakconfig"
    config.write_text(
        config.read_text().replace("[cells]\n", "[cells]\n  assets = assets\n", 1)
    )
    result = await yak.run("//crates/gen")
    assert result.stdout == "assets=top sub\n"

    (yak.cwd / "assets" / "sub" / "sub.txt").write_text("edited\n")
    result = await yak.run("//crates/gen")
    assert result.stdout == "assets=top edited\n"


@yak_test(data_dir="linksmetadata")
async def test_generate_passes_links_metadata_to_build_scripts(yak: Yak) -> None:
    # `sys` sets `links = "answer"`, and its build script reports a directory
    # in its `OUT_DIR` and one of its own files. The build script of `app`
    # reads them through `DEP_ANSWER_INCLUDE` and `DEP_ANSWER_DATA_DIR`.
    await yak.generate()
    result = await yak.run("//app")
    assert result.stdout == "answer=42 (forty-two)\n"


@yak_test(data_dir="sharedlib")
async def test_generate_runs_a_binary_that_loads_a_build_script_shared_library(
    yak: Yak,
) -> None:
    # The build script of `seven` compiles `libseven` into its `OUT_DIR` and
    # links it, as Cargo lets a build script do. Cargo puts the directory on
    # the dynamic library path of the programs it runs, and so does yak.
    await yak.generate()
    result = await yak.run("//app")
    assert result.stdout == "seven=7\n"
    await yak.test("//seven:seven-unittest")


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
    assert "root//seven:seven" in built
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
    targets = (await yak.targets("//probe:")).stdout.split()
    assert "root//probe:probe-example-plugin" in targets
    assert "root//probe:probe-probe-unittest" in targets
    # `extra` needs a feature that is off, so Cargo skips it.
    assert "root//probe:extra" not in targets

    # The tests read `shared.txt` from the workspace's directory, which the
    # build file at the root exports.
    (yak.cwd / "YAK").write_text(
        WORKSPACE_BUILD_FILE
        + '\nexport_file(name = "shared.txt", visibility = ["//probe:"])\n'
    )
    probe_build_file = yak.cwd / "probe" / "YAK"
    probe_build_file.write_text(
        MEMBER_BUILD_FILE.replace(
            "cargo_package()", 'cargo_package(test_data = ["//:shared.txt"])'
        )
    )
    # `//probe` names the binary, which lists the package's tests.
    result = await yak.test("//probe")
    summary = re.sub("\x1b\\[[0-9;]*m", "", result.stderr)
    assert "Pass 4. Fail 0." in summary, result.stderr

    # A test reads only its package's files and the files it declares.
    probe_build_file.write_text(MEMBER_BUILD_FILE)
    await expect_failure(
        yak.test("//probe:probe-unittest"),
        stderr_regex="reads_a_workspace_file_at_run_time",
    )

    probe_build_file.write_text(
        MEMBER_BUILD_FILE.replace(
            "cargo_package()", 'cargo_package(test_data = ["shared.txt"])'
        )
    )
    await expect_failure(
        yak.targets("//probe:"),
        stderr_regex="`test_data` takes labels of targets, such as",
    )

    # The manifest no longer declares yak's inputs.
    probe_build_file.write_text(MEMBER_BUILD_FILE)
    manifest = yak.cwd / "probe" / "Cargo.toml"
    manifest.write_text(
        manifest.read_text() + '\n[package.metadata.yak]\ntest-data = ["../shared.txt"]\n'
    )
    await expect_failure(
        yak.targets("//probe:"),
        stderr_regex="`\\[package.metadata.yak\\]` of package `probe` is no longer read",
    )


@pytest.mark.skipif(shutil.which("go") is None, reason="needs Go")
@yak_test(data_dir="gomodules")
async def test_generate_builds_each_go_module(yak: Yak) -> None:
    await yak.generate()
    assert (yak.cwd / "YAK").read_text() == (
        'load("@gomod//:module.bzl", "go_module")\n\ngo_module()\n'
    )
    assert (yak.cwd / "services" / "api" / "YAK").read_text() == (
        'load("@gomod_services_api//:module.bzl", "go_module")\n\ngo_module()\n'
    )
    # Go leaves modules in `testdata` out of `./...`, and so does `yak generate`.
    assert not (yak.cwd / "services" / "api" / "testdata" / "fixture" / "YAK").exists()
    config = (yak.cwd / ".yakconfig").read_text()
    assert "[external_cell_gomod_services_api]\n  module = services/api/go.mod\n" in config

    result = await yak.run("//:hello")
    assert result.stdout == "hello from the root module\n"
    result = await yak.run("//services/api:api")
    assert result.stdout == "api serves 3\n"
    result = await yak.test("//services/api/...")
    summary = re.sub("\x1b\\[[0-9;]*m", "", result.stderr)
    assert "Pass 1. Fail 0." in summary, result.stderr

    result = await yak.generate()
    assert "`YAK` is up to date" in result.stderr
    assert "`services/api/YAK` is up to date" in result.stderr
    assert (yak.cwd / ".yakconfig").read_text() == config


@yak_test(data_dir="gomodules")
async def test_generate_skips_ignored_go_modules(yak: Yak) -> None:
    (yak.cwd / ".yakconfig").write_text(
        "[cells]\n  root = .\n\n[project]\n  ignore = services\n"
    )
    await yak.generate()
    assert (yak.cwd / "YAK").exists()
    assert not (yak.cwd / "services" / "api" / "YAK").exists()
    assert "gomod_services_api" not in (yak.cwd / ".yakconfig").read_text()
