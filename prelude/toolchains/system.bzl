# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//tests:test_toolchain.bzl", "noop_test_toolchain")
load("@prelude//toolchains:cxx.bzl", "system_cxx_toolchain")
load("@prelude//toolchains:erlang.bzl", "system_erlang_toolchain")
load("@prelude//toolchains:genrule.bzl", "system_genrule_toolchain")
load("@prelude//toolchains:haskell.bzl", "system_haskell_toolchain")
load("@prelude//toolchains:ocaml.bzl", "system_ocaml_toolchain")
load(
    "@prelude//toolchains:python.bzl",
    "remote_python_toolchain",
    "system_python_bootstrap_toolchain",
    "system_python_wheel_toolchain",
)
load("@prelude//toolchains:remote_test_execution.bzl", "remote_test_execution_toolchain")
load("@prelude//toolchains:rust.bzl", "system_rust_toolchain")
load("@prelude//toolchains:zip_file.bzl", "zip_file_toolchain")
load("@prelude//toolchains/go:system_go_bootstrap_toolchain.bzl", "system_go_bootstrap_toolchain")
load("@prelude//toolchains/go:system_go_toolchain.bzl", "system_go_toolchain")

def system_toolchains():
    """
    Declares a toolchain for each language that the prelude supports, using the
    compilers and tools on `PATH`. A project that needs other tools declares its
    toolchains with the rules that this macro calls.

    Each toolchain gets the identity of its tool from `[tool_identity]`, which
    the daemon computes from the tool's version output, so the actions that run
    a tool run again when the tool changes.
    """
    system_cxx_toolchain(
        name = "cxx",
        tool_identity = read_root_config("tool_identity", "clang", ""),
        visibility = ["PUBLIC"],
    )

    system_genrule_toolchain(
        name = "genrule",
        visibility = ["PUBLIC"],
    )

    system_go_toolchain(
        name = "go",
        tool_identity = read_root_config("tool_identity", "go", ""),
        visibility = ["PUBLIC"],
    )

    system_go_bootstrap_toolchain(
        name = "go_bootstrap",
        tool_identity = read_root_config("tool_identity", "go", ""),
        visibility = ["PUBLIC"],
    )

    system_haskell_toolchain(
        name = "haskell",
        visibility = ["PUBLIC"],
    )

    system_ocaml_toolchain(
        name = "ocaml",
        visibility = ["PUBLIC"],
    )

    remote_python_toolchain(
        name = "python",
        bootstrap = False,
        visibility = ["PUBLIC"],
    )

    system_python_bootstrap_toolchain(
        name = "python_bootstrap",
        visibility = ["PUBLIC"],
    )

    system_python_wheel_toolchain(
        name = "python_wheel",
        visibility = ["PUBLIC"],
    )

    system_rust_toolchain(
        name = "rust",
        tool_identity = read_root_config("tool_identity", "rustc", ""),
        default_edition = "2021",
        visibility = ["PUBLIC"],
    )

    remote_test_execution_toolchain(
        name = "remote_test_execution",
        visibility = ["PUBLIC"],
    )

    noop_test_toolchain(
        name = "test",
        visibility = ["PUBLIC"],
    )

    zip_file_toolchain(
        name = "zip_file",
        visibility = ["PUBLIC"],
    )

    system_erlang_toolchain(
        name = "erlang-default",
        visibility = ["PUBLIC"],
    )
