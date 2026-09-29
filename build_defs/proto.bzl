# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Rules for the protobuf crates of this repository."""

load(":rust.bzl", "rust_binary", "rust_library")

def rust_protobuf_library(
        name,
        srcs,
        build_script,
        protos = [],
        deps = [],
        doctests = True,
        build_env = {},
        proto_srcs = None):
    """Compiles protobuf definitions into a Rust library.

    `build_script` is the crate's Cargo build script, which calls
    `yak_protoc_dev`. It runs as the binary `<name>-build`, the genrule
    `<name>-proto` holds its output, and the library reads that output
    through `OUT_DIR`, as it would under Cargo.

    Args:
        name: The library target and crate name.
        srcs: The library sources.
        build_script: The Cargo build script that generates the code.
        protos: Files to place in the build script's working directory.
        deps: Dependencies of the library.
        doctests: Whether to run the library's doctests.
        build_env: Extra environment for the build script.
        proto_srcs: A `proto_srcs` target, exposed to the build script as
            `YAK_PROTO_SRCS`.
    """
    build_name = name + "-build"
    proto_name = name + "-proto"

    rust_binary(
        name = build_name,
        srcs = [build_script],
        crate_root = build_script,
        deps = ["//app/yak_protoc_dev:yak_protoc_dev"],
    )

    env = dict(build_env)
    env["PROTOC"] = "$(exe //third-party/proto:protoc)"
    env["PROTOC_INCLUDE"] = "$(location //third-party/proto:google_protobuf)"
    if proto_srcs:
        env["YAK_PROTO_SRCS"] = "$(location {})".format(proto_srcs)

    native.genrule(
        name = proto_name,
        srcs = protos + ["//third-party/proto:google_protobuf"],
        out = ".",
        cmd = "$(exe :{})".format(build_name),
        env = env,
    )

    rust_library(
        name = name,
        srcs = srcs,
        doctests = doctests,
        env = {
            # The crate includes the generated code from OUT_DIR.
            "OUT_DIR": "$(location :{})".format(proto_name),
        },
        deps = [
            "//third-party/rust:prost",
            "//third-party/rust:tonic",
            "//third-party/rust:tonic-prost",
        ] + deps,
        rustc_flags = ["-Aunused-crate-dependencies"],
    )

ProtoSrcsInfo = provider(fields = ["srcs"])

def _proto_srcs_impl(ctx):
    srcs = {src.basename: src for src in ctx.attrs.srcs}
    for dep in ctx.attrs.deps:
        for src in dep[ProtoSrcsInfo].srcs:
            if src.basename in srcs:
                fail("Duplicate src:", src.basename)
            srcs[src.basename] = src
    out = ctx.actions.copied_dir(ctx.attrs.name, srcs, has_content_based_path = False)
    return [DefaultInfo(default_output = out), ProtoSrcsInfo(srcs = srcs.values())]

# Collects `.proto` files, and those of its `deps`, into one directory.
proto_srcs = rule(
    impl = _proto_srcs_impl,
    attrs = {
        "deps": attrs.list(attrs.dep(), default = []),
        "srcs": attrs.list(attrs.source(), default = []),
    },
)
