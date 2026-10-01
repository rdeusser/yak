# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Tests rules of the bundled prelude with the toolchains that
# `system_toolchains()` finds on the host. Each package in the test data
# uses a rule and declares `py_assertion` targets, whose actions run a Python
# script against the rule's outputs, so building a package runs its checks.

import shutil
import sys

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test

COMMAND_ALIAS_PACKAGES = [
    "cross_build",
    "env",
    "exe_script",
    "executable_name",
    "json",
    "multiple_args",
    "quoting",
    "relative_paths",
    "resources",
    "single_arg_exe",
    "symlinks",
]

_needs_go = pytest.mark.skipif(shutil.which("go") is None, reason="needs Go")

# These packages link with the C++ toolchain. On macOS the Go linker then fails
# with "combining dwarf failed: no room to add dwarf info".
_external_link = pytest.mark.skipif(
    sys.platform == "darwin",
    reason="Go external linking with the system C++ toolchain fails on macOS",
)

GO_PACKAGES = [
    "asm/basic",
    "asm/with_header",
    "binary",
    "binary_coverage",
    "embed/generated_directory",
    "embed/generated_file",
    "embed/named_embed",
    "embed/plain_directory",
    "embed/plain_file",
    "go_exported_library",
    "go_test/basic",
    "go_test/coverage",
    "go_test/resources",
    "go_test/target_under_test",
] + [
    pytest.param(package, marks=_external_link)
    for package in [
        "cgo/c_files",
        "cgo/cxx_library",
        "cgo/embedded",
        "cgo/include_dir",
        "link_mode",
    ]
]


@yak_test()
@pytest.mark.parametrize("package", COMMAND_ALIAS_PACKAGES)
async def test_command_alias(yak: Yak, package: str) -> None:
    await yak.build(f"root//command_alias/{package}/...")


@_needs_go
@yak_test()
@pytest.mark.parametrize("package", GO_PACKAGES)
async def test_go(yak: Yak, package: str) -> None:
    await yak.build(f"root//go/{package}/...")


@yak_test()
async def test_cxx_flags(yak: Yak) -> None:
    await yak.build("root//cxx_flags/...")


@yak_test()
async def test_cxx_flags_rejects_other_targets(yak: Yak) -> None:
    await expect_failure(
        yak.build("root//cxx_flags_errors:consumer"),
        stderr_regex="CxxFlagsInfo",
    )


@yak_test()
async def test_zip_file(yak: Yak) -> None:
    await yak.build("root//zip_file/...")


@yak_test()
@pytest.mark.parametrize(
    "target, message",
    [
        ("duplicate_entry", "Duplicate entry `a.txt` comes from"),
        ("duplicate_srcs", "Entry `a.txt` comes from both"),
        ("invalid_exclude", "pattern `\\(` is not a valid regular expression"),
    ],
)
async def test_zip_file_errors(yak: Yak, target: str, message: str) -> None:
    await expect_failure(
        yak.build(f"root//zip_file_errors:{target}"),
        stderr_regex=message,
    )
