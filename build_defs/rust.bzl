# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Rust rules for the yak build of this repository.

Each macro calls the prelude rule of the same name with `--cfg=yak_build`
added to `rustc_flags`, so code can check `cfg(yak_build)` where the yak
and Cargo builds differ. Targets are public unless they set `visibility`.
"""

load("@prelude//rust:linkable_symbol.bzl", _prelude_rust_linkable_symbol = "rust_linkable_symbol")

_CFG_YAK_BUILD = "--cfg=yak_build"

def rust_library(rustc_flags = [], visibility = ["PUBLIC"], **kwargs):
    native.rust_library(rustc_flags = rustc_flags + [_CFG_YAK_BUILD], visibility = visibility, **kwargs)

def rust_binary(rustc_flags = [], visibility = ["PUBLIC"], **kwargs):
    native.rust_binary(rustc_flags = rustc_flags + [_CFG_YAK_BUILD], visibility = visibility, **kwargs)

def rust_test(rustc_flags = [], visibility = ["PUBLIC"], **kwargs):
    native.rust_test(rustc_flags = rustc_flags + [_CFG_YAK_BUILD], visibility = visibility, **kwargs)

def rust_linkable_symbol(visibility = ["PUBLIC"], **kwargs):
    _prelude_rust_linkable_symbol(visibility = visibility, rust_library_macro = rust_library, **kwargs)
