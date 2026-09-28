# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Tests rules of the bundled prelude with the toolchains that
# `system_demo_toolchains()` finds on the host. Each package in the test data
# uses a rule and declares `py_assertion` targets, whose actions run a Python
# script against the rule's outputs, so building a package runs its checks.

import shutil
import sys

import pytest
from e2e_util.api.buck import Buck
from e2e_util.buck_workspace import buck_test

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
    reason="Go external linking with the demo C++ toolchain fails on macOS",
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


@buck_test()
@pytest.mark.parametrize("package", COMMAND_ALIAS_PACKAGES)
async def test_command_alias(buck: Buck, package: str) -> None:
    await buck.build(f"root//command_alias/{package}/...")


@_needs_go
@buck_test()
@pytest.mark.parametrize("package", GO_PACKAGES)
async def test_go(buck: Buck, package: str) -> None:
    await buck.build(f"root//go/{package}/...")
