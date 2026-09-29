# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//csharp:toolchain.bzl", "CSharpToolchainInfo")
load("@prelude//go:toolchain.bzl", "GoToolchainInfo")
load("@prelude//go_bootstrap:go_bootstrap.bzl", "GoBootstrapToolchainInfo")
load("@prelude//haskell:toolchain.bzl", "HaskellPlatformInfo", "HaskellToolchainInfo")
load("@prelude//python:python_wheel_toolchain.bzl", "PythonWheelToolchainInfo")
load("@prelude//python:toolchain.bzl", "PythonPlatformInfo", "PythonToolchainInfo")
load("@prelude//python/cython:cython_toolchain.bzl", "CythonToolchainInfo")
load("@prelude//python_bootstrap:python_bootstrap.bzl", "PythonBootstrapToolchainInfo")
load("@prelude//rust:rust_toolchain.bzl", "RustToolchainInfo")
load("@prelude//tests:remote_test_execution_toolchain.bzl", "RemoteTestExecutionToolchainInfo")
load("@prelude//tests:test_toolchain.bzl", "TestToolchainInfo")
load("@prelude//zip_file:zip_file_toolchain.bzl", "ZipFileToolchainInfo")

def _toolchain(lang: str, providers: list[typing.Any], *, default: typing.Any = None) -> Attr:
    return attrs.toolchain_dep(default = default or ("toolchains//:" + lang), providers = providers)

def _csharp_toolchain():
    return _toolchain("csharp", [CSharpToolchainInfo])

def _cython_toolchain():
    return _toolchain("cython", [CythonToolchainInfo])

def _cxx_toolchain():
    # `CxxToolchainInfo, CxxPlatformInfo`, but python doesn't require it
    lang = "cxx"

    # Portions of the macro layers use the `default_deps` attribute to set this
    # to the `:cxx_no_default_deps` option. If a targets has within_view checks
    # that don't list `toolchains//:` they will experience an error. We can avoid
    # this by ensuring the `within_deps` checks inside yak see either of the
    # toolchains below as a possible default value, even though one of them is
    # never able to be selected in this expression.
    return _toolchain(lang, [], default = select({"DEFAULT": "toolchains//:" + lang, "config//:none": "toolchains//:cxx_no_default_deps"}))

def _go_toolchain():
    return _toolchain("go", [GoToolchainInfo])

def _go_bootstrap_toolchain():
    return _toolchain("go_bootstrap", [GoBootstrapToolchainInfo])

def _haskell_toolchain():
    return _toolchain("haskell", [HaskellToolchainInfo, HaskellPlatformInfo])

def _python_toolchain():
    return _toolchain("python", [PythonToolchainInfo, PythonPlatformInfo])

def _python_bootstrap_toolchain():
    return _toolchain("python_bootstrap", [PythonBootstrapToolchainInfo])

def _python_wheel_toolchain():
    return _toolchain("python_wheel", [PythonWheelToolchainInfo])

def _rust_toolchain():
    return _toolchain("rust", [RustToolchainInfo])

def _zip_file_toolchain():
    return _toolchain("zip_file", [ZipFileToolchainInfo])

def _remote_test_execution_toolchain():
    return _toolchain("remote_test_execution", [RemoteTestExecutionToolchainInfo])

def _test_toolchain():
    return _toolchain("test", [TestToolchainInfo])

toolchains_common = struct(
    csharp = _csharp_toolchain,
    cxx = _cxx_toolchain,
    cython = _cython_toolchain,
    go = _go_toolchain,
    go_bootstrap = _go_bootstrap_toolchain,
    haskell = _haskell_toolchain,
    python = _python_toolchain,
    python_bootstrap = _python_bootstrap_toolchain,
    python_wheel = _python_wheel_toolchain,
    test_toolchain = _test_toolchain,
    rust = _rust_toolchain,
    zip_file = _zip_file_toolchain,
    remote_test_execution = _remote_test_execution_toolchain,
)
