# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import subprocess
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException
from e2e_util.yak_workspace import yak_test

TESTS = ["root//app:test", "root//other:test"]


def git(cwd: Path, *argv: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *argv],
        cwd=cwd,
        check=True,
        capture_output=True,
    )


def commit_project(yak: Yak) -> None:
    git(yak.cwd, "init", "-q", "-b", "main")
    git(yak.cwd, "add", ".")
    git(yak.cwd, "commit", "-q", "-m", "base")


async def run_changed(yak: Yak, revision: str = "HEAD") -> tuple[list[str], str]:
    """Tests what the changes since `revision` affect, and returns the tests
    that ran with yak's output. Every test fails, so yak reports each test it
    ran."""
    try:
        result = await yak.test("--changed-since", revision, "//...")
        stderr = result.stderr
    except YakException as e:
        stderr = e.stderr
    return [t for t in TESTS if f"Fail: {t}" in stderr], stderr


def append(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a") as f:
        f.write(text)


@yak_test()
async def test_no_change_tests_nothing(yak: Yak) -> None:
    commit_project(yak)
    ran, stderr = await run_changed(yak)
    assert ran == []
    assert "Testing 0 of 4 matched targets" in stderr


@yak_test()
async def test_a_source_change_tests_the_targets_that_depend_on_it(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "lib" / "a.txt", "changed\n")
    ran, _ = await run_changed(yak)
    assert ran == ["root//app:test"]


@yak_test()
async def test_a_file_that_no_target_reads_tests_nothing(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "app" / "notes.md", "changed\n")
    append(yak.cwd / "docs" / "new.md", "new\n")
    ran, _ = await run_changed(yak)
    assert ran == []


@yak_test()
async def test_a_new_file_tests_the_package_that_holds_it(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "lib" / "b.txt", "new\n")
    ran, _ = await run_changed(yak)
    assert ran == ["root//app:test"]


@yak_test()
async def test_a_removed_file_tests_the_package_that_held_it(yak: Yak) -> None:
    append(yak.cwd / "other" / "old.md", "old\n")
    commit_project(yak)
    (yak.cwd / "other" / "old.md").unlink()
    ran, _ = await run_changed(yak)
    assert ran == ["root//other:test"]


@yak_test()
async def test_a_build_file_change_tests_its_package(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "other" / "YAK.fixture", "# changed\n")
    ran, _ = await run_changed(yak)
    assert ran == ["root//other:test"]


@yak_test()
async def test_a_rule_change_tests_the_targets_of_that_rule(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "rules" / "other.bzl", "# changed\n")
    ran, _ = await run_changed(yak)
    assert ran == ["root//other:test"]


@yak_test()
async def test_a_change_to_a_loaded_file_tests_every_target_that_loads_it(
    yak: Yak,
) -> None:
    commit_project(yak)
    append(yak.cwd / "rules" / "test.bzl", "# changed\n")
    ran, _ = await run_changed(yak)
    assert ran == TESTS


@yak_test()
async def test_a_configuration_change_tests_everything(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / ".yakconfig", "[user]\n    value = 1\n")
    ran, stderr = await run_changed(yak)
    assert ran == TESTS
    assert "the configuration changed at `user.value` in cell `root`" in stderr


@yak_test()
async def test_an_ignored_local_configuration_is_no_change(yak: Yak) -> None:
    append(yak.cwd / ".gitignore", ".yakconfig.local\n")
    commit_project(yak)
    append(yak.cwd / ".yakconfig.local", "[user]\n    value = 1\n")
    ran, _ = await run_changed(yak)
    assert ran == []


@yak_test()
async def test_a_configuration_target_change_tests_everything(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "config" / "YAK.fixture", "# changed\n")
    ran, stderr = await run_changed(yak)
    assert ran == TESTS
    assert "the configuration target `root//config:setting` may have changed" in stderr


@yak_test()
async def test_a_path_in_the_select_all_setting_tests_everything(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "tools" / "lint.sh", "true\n")
    ran, stderr = await run_changed(yak)
    assert ran == TESTS
    assert "`tools/lint.sh` changed" in stderr


@yak_test()
async def test_a_rust_toolchain_file_tests_everything(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "rust-toolchain.toml", '[toolchain]\nchannel = "stable"\n')
    ran, _ = await run_changed(yak)
    assert ran == TESTS


@yak_test()
async def test_changes_count_from_the_merge_base(yak: Yak) -> None:
    commit_project(yak)
    git(yak.cwd, "checkout", "-q", "-b", "topic")
    append(yak.cwd / "lib" / "a.txt", "topic\n")
    git(yak.cwd, "commit", "-q", "-am", "topic")
    git(yak.cwd, "checkout", "-q", "main")
    append(yak.cwd / "other" / "other.txt", "main\n")
    git(yak.cwd, "commit", "-q", "-am", "main")
    git(yak.cwd, "checkout", "-q", "topic")
    ran, _ = await run_changed(yak, "main")
    assert ran == ["root//app:test"]


@yak_test()
async def test_a_package_file_tests_the_packages_below_it(yak: Yak) -> None:
    commit_project(yak)
    append(yak.cwd / "PACKAGE", "")
    ran, _ = await run_changed(yak)
    assert ran == TESTS


@yak_test()
async def test_a_removed_dependency_tests_its_dependents(yak: Yak) -> None:
    commit_project(yak)
    (yak.cwd / "lib" / "YAK.fixture").write_text(
        'load("//rules:test.bzl", "lib")\n\nlib(name = "renamed")\n'
    )
    _, stderr = await run_changed(yak)
    assert "Testing 2 of 4 matched targets" in stderr
    assert "root//lib:lib" in stderr
