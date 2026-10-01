# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test

pytestmark = pytest.mark.skipif(shutil.which("go") is None, reason="needs Go")

DEP_MODULE = "example.com/dep"
DEP_VERSION = "v1.0.0"


def write_module_proxy(module_dir: Path, proxy: Path) -> None:
    """Writes a `GOPROXY=file://` directory that serves the module in `module_dir` as
    `DEP_MODULE` at `DEP_VERSION`."""
    versions = proxy / DEP_MODULE / "@v"
    versions.mkdir(parents=True)
    (versions / "list").write_text(f"{DEP_VERSION}\n")
    (versions / f"{DEP_VERSION}.info").write_text(
        f'{{"Version":"{DEP_VERSION}","Time":"2026-01-01T00:00:00Z"}}\n'
    )
    shutil.copy(module_dir / "go.mod", versions / f"{DEP_VERSION}.mod")
    with zipfile.ZipFile(versions / f"{DEP_VERSION}.zip", "w") as archive:
        for path in sorted(module_dir.rglob("*")):
            if path.is_file():
                name = path.relative_to(module_dir).as_posix()
                archive.write(path, f"{DEP_MODULE}@{DEP_VERSION}/{name}")


def use_module_proxy(yak: Yak) -> None:
    """Serves the module in `dep/` from a module proxy in a temporary directory, with a
    module cache of its own, and removes `dep/` from the project."""
    root = Path(tempfile.mkdtemp(prefix="go_cell_"))
    write_module_proxy(yak.cwd / "dep", root / "proxy")
    shutil.rmtree(yak.cwd / "dep")
    yak.set_env("GOPROXY", (root / "proxy").as_uri())
    yak.set_env("GOMODCACHE", str(root / "modcache"))
    yak.set_env("GOFLAGS", "-modcacherw")
    yak.set_env("GOSUMDB", "off")
    yak.set_env("GOTOOLCHAIN", "local")


@yak_test(data_dir="module")
async def test_go_cell_runs_a_binary(yak: Yak) -> None:
    use_module_proxy(yak)
    result = await yak.run("//:app")
    assert result.stdout == "Hello, DEP v1.0.0!\n"

    targets = await yak.targets("gomod//...")
    assert targets.stdout.splitlines() == [
        "gomod//:example.com/dep",
        "gomod//:example.com/dep/internal/upper",
        "gomod//example.com/dep@v1.0.0:example.com/dep",
        "gomod//example.com/dep@v1.0.0:example.com/dep/internal/upper",
    ]


@yak_test(data_dir="module")
async def test_go_cell_runs_internal_and_external_tests(yak: Yak) -> None:
    use_module_proxy(yak)
    result = await yak.test("//...")
    summary = re.sub("\x1b\\[[0-9;]*m", "", result.stderr)
    assert "Pass 1. Fail 0." in summary, result.stderr

    # The test target holds the package's tests and its external tests.
    for name, test in [
        ("greet_test.go", "TestMessage"),
        ("greet_ext_test.go", "TestGreet"),
    ]:
        path = yak.cwd / "greet" / name
        source = path.read_text()
        path.write_text(source.replace("\tif ", "\tif true || "))
        await expect_failure(yak.test("//:greet-test"), stderr_regex=test)
        path.write_text(source)


@yak_test(data_dir="module")
async def test_go_cell_declares_a_package_below_the_module_root(yak: Yak) -> None:
    use_module_proxy(yak)
    targets = await yak.targets("//...")
    assert targets.stdout.splitlines() == [
        "root//:app",
        "root//:greet",
        "root//:greet-test",
        "root//tools:version",
    ]
    result = await yak.run("//tools:version")
    assert result.stdout == "tools version 1\n"

    (yak.cwd / "tools" / "YAK.fixture").write_text("")
    await expect_failure(
        yak.targets("//:"),
        stderr_regex="The build file of `tools` does not call `go_package\\(\\)`",
    )


@yak_test(data_dir="module")
async def test_go_cell_follows_an_edited_import(yak: Yak) -> None:
    use_module_proxy(yak)
    await yak.run("//:app")

    # `greet` imports `dep` now, so its target needs the dependency.
    greet = yak.cwd / "greet" / "greet.go"
    greet.write_text(
        greet.read_text()
        .replace('"strings"\n', '"strings"\n\n\t"example.com/dep"\n')
        .replace(
            "return strings.ReplaceAll(strings.TrimSpace(message), \"NAME\", name)",
            'return strings.ReplaceAll(strings.TrimSpace(message), "NAME", name) + " (" + dep.Name() + ")"',
        )
    )
    result = await yak.run("//:app")
    assert result.stdout == "Hello, DEP v1.0.0! (DEP v1.0.0)\n"


@yak_test(data_dir="module")
async def test_go_cell_rejects_a_vendored_module(yak: Yak) -> None:
    use_module_proxy(yak)
    (yak.cwd / "vendor").mkdir()
    (yak.cwd / "vendor" / "modules.txt").write_text("")
    await expect_failure(
        yak.targets("//:"),
        stderr_regex="vendors its dependencies in `vendor/`",
    )


def commit_project(yak: Yak) -> None:
    for argv in [
        ["init", "-q", "-b", "main"],
        ["add", "."],
        ["commit", "-q", "-m", "base"],
    ]:
        subprocess.run(
            ["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *argv],
            cwd=yak.cwd,
            check=True,
            capture_output=True,
        )


@yak_test(data_dir="module")
async def test_changed_since_follows_the_inputs_of_the_go_cell(yak: Yak) -> None:
    use_module_proxy(yak)
    (yak.cwd / "tools" / "notes.txt").write_text("notes\n")
    (yak.cwd / "tools" / "YAK.fixture").write_text(
        (yak.cwd / "tools" / "YAK.fixture").read_text()
        + '\nexport_file(name = "notes.txt")\n'
    )
    commit_project(yak)

    # A file that the cell does not read leaves `module.bzl` unchanged.
    (yak.cwd / "tools" / "notes.txt").write_text("more notes\n")
    result = await yak.test("--changed-since", "HEAD", "//...")
    assert "NO TESTS RAN" in result.stderr, result.stderr

    # A Go file can change its imports, so every package that loads
    # `module.bzl` can change, including the one with `greet-test`.
    main = yak.cwd / "tools" / "version" / "main.go"
    main.write_text(main.read_text().replace('"fmt"', '"fmt"\n\t"strings"').replace(
        'fmt.Println("tools version 1")', 'fmt.Println(strings.TrimSpace(" tools version 1 "))'
    ))
    result = await yak.test("--changed-since", "HEAD", "//...")
    summary = re.sub("\x1b\\[[0-9;]*m", "", result.stderr)
    assert "Pass 1. Fail 0." in summary, result.stderr
