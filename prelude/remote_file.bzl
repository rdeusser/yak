# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//:http_file.bzl", "http_file_shared")
load("@prelude//utils:utils.bzl", "value_or")

# Implementation of the `remote_file` build rule.
def remote_file_impl(ctx: AnalysisContext) -> list[Provider]:
    return http_file_shared(
        ctx.actions,
        name = value_or(ctx.attrs.out, ctx.label.name),
        url = ctx.attrs.url,
        is_executable = ctx.attrs.type == "executable",
        is_exploded_zip = ctx.attrs.type == "exploded_zip",
        unzip_tool = ctx.attrs._unzip_tool[RunInfo],
        sha1 = ctx.attrs.sha1,
        sha256 = ctx.attrs.sha256,
        size_bytes = ctx.attrs.size_bytes,
        has_content_based_path = ctx.attrs.has_content_based_path,
    )
