# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import platform
import re
import shutil
import tempfile
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


async def ran_build_actions(yak: Yak) -> list[str]:
    """The identities of the build actions that the last command ran."""
    result = await yak.log("what-ran")
    return [
        line.split("\t")[1]
        for line in result.stdout.splitlines()
        if line.startswith("build\t")
    ]


@yak_test(data_dir="rules")
async def test_the_daemon_computes_tool_identities(yak: Yak) -> None:
    result = await yak.audit_config("tool_identity")
    identities = dict(re.findall(r"^\s+(\w+) = (\S+)$", result.stdout, re.MULTILINE))
    assert sorted(identities) == ["clang", "go", "rustc"]
    for identity in identities.values():
        assert re.fullmatch("[0-9a-f]{16}", identity), identities

    # A `-c` of the client wins over the computed value.
    result = await yak.audit_config(
        "tool_identity.rustc", "-c", "tool_identity.rustc=pinned"
    )
    assert "rustc = pinned" in result.stdout


@yak_test(data_dir="rules")
async def test_a_changed_rustc_runs_the_rust_actions_again(yak: Yak) -> None:
    await yak.build("//:rust_lib")

    await yak.build("//:rust_lib", "-c", "tool_identity.rustc=other")
    assert any("//:rust_lib" in a for a in await ran_build_actions(yak))

    # The identities of other tools are not inputs of the Rust actions.
    await yak.build(
        "//:rust_lib",
        "-c",
        "tool_identity.rustc=other",
        "-c",
        "tool_identity.go=other",
        "-c",
        "tool_identity.clang=other",
    )
    assert await ran_build_actions(yak) == []


@pytest.mark.skipif(shutil.which("go") is None, reason="needs Go")
@yak_test(data_dir="rules")
async def test_a_changed_go_runs_the_go_actions_again(yak: Yak) -> None:
    await yak.build("//:go_lib")

    await yak.build("//:go_lib", "-c", "tool_identity.go=other")
    ran = await ran_build_actions(yak)
    assert any("//:go_lib" in a for a in ran), ran
    assert any("go_copy_goroot" in a for a in ran), ran


def log_runs_of(yak: Yak, tool: str) -> Path:
    """Puts a wrapper of `tool` first on the `PATH` of yak and its daemon, which appends the
    arguments of each run to the returned file."""
    real = shutil.which(tool)
    assert real is not None
    wrappers = Path(tempfile.mkdtemp(prefix="tool_identity_"))
    log = wrappers / f"{tool}.log"
    log.touch()
    wrapper = wrappers / tool
    wrapper.write_text(f'#!/bin/sh\necho "$@" >> "{log}"\nexec "{real}" "$@"\n')
    wrapper.chmod(0o755)
    yak.set_env("PATH", f"{wrappers}{os.pathsep}{os.environ['PATH']}")
    return log


def runs(log: Path, subcommand: str) -> int:
    return sum(
        1 for line in log.read_text().splitlines() if line.split()[:1] == [subcommand]
    )


_needs_posix = pytest.mark.skipif(
    platform.system() == "Windows", reason="the tool wrappers are shell scripts"
)


@_needs_posix
@yak_test(data_dir="cargo")
async def test_a_changed_rustc_computes_the_cargo_cell_again(yak: Yak) -> None:
    log = log_runs_of(yak, "cargo")
    await yak.generate()
    await yak.targets("//hello:")
    metadata_runs = runs(log, "metadata")
    assert metadata_runs >= 1

    await yak.targets("//hello:", "-c", "tool_identity.go=other")
    assert runs(log, "metadata") == metadata_runs

    await yak.targets("//hello:", "-c", "tool_identity.rustc=other")
    assert runs(log, "metadata") == metadata_runs + 1


@_needs_posix
@pytest.mark.skipif(shutil.which("go") is None, reason="needs Go")
@yak_test(data_dir="go")
async def test_a_changed_go_computes_the_go_cell_again(yak: Yak) -> None:
    log = log_runs_of(yak, "go")
    yak.set_env("GOTOOLCHAIN", "local")
    await yak.generate()
    await yak.targets("//:")
    list_runs = runs(log, "list")
    assert list_runs >= 1

    await yak.targets("//:", "-c", "tool_identity.rustc=other")
    assert runs(log, "list") == list_runs

    await yak.targets("//:", "-c", "tool_identity.go=other")
    assert runs(log, "list") > list_runs
