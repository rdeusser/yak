# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def cxx_toolchain_macro_impl(cxx_toolchain_rule = None, **kwargs):
    # An explicit `generate_linker_maps` attribute on the cxx_toolchain() target
    # takes priority over the `cxx.linker_map_enabled` yakconfig.
    if "generate_linker_maps" not in kwargs:
        kwargs["generate_linker_maps"] = read_root_config("cxx", "linker_map_enabled", "").lower() == "true"

    bitcode = read_root_config("cxx", "bitcode")
    if bitcode != None:
        if bitcode.lower() == "false":
            kwargs["object_format"] = "native"
        elif bitcode.lower() == "true":
            kwargs["object_format"] = "bitcode"
        elif bitcode.lower() == "embed":
            kwargs["object_format"] = "embedded-bitcode"
        else:
            kwargs["object_format"] = "native"

    cxx_toolchain_rule(**kwargs)
