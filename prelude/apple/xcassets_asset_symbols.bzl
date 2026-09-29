# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


load("@prelude//cxx:cxx_sources.bzl", "CxxSrcWithFlags")

def meta_xcassets_asset_symbol_usage_providers_and_subtargets(
    ctx: AnalysisContext, cxx_srcs: list[CxxSrcWithFlags], swift_srcs: list[CxxSrcWithFlags]
) -> (list[Provider], dict[str, list[Provider]]):
    return [], {}
