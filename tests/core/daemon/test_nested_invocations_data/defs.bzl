# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _normal_impl(ctx):
    out = ctx.actions.declare_output("out.txt", has_content_based_path = False)

    cmd = cmd_args(
        ctx.attrs.yak_path,
        "build",
        "root//:trivial",
        "-c",
        "nested.yak_path=" + ctx.attrs.yak_path,
        "--out",
        out.as_output(),
    )
    ctx.actions.run(
        cmd,
        local_only = True,
        category = "run",
    )
    return [DefaultInfo(default_output = out)]

def _trace_impl(ctx):
    trace_out = ctx.actions.declare_output("trace_out.txt", has_content_based_path = False)
    nested_out = ctx.actions.declare_output("nested_out.txt", has_content_based_path = False)

    script = ctx.actions.write(
        "script.py",
        """
import subprocess
import sys

yak_path = sys.argv[1]
subprocess.run([yak_path, "debug", "trace-io", "enable"])
    """,
        has_content_based_path = False,
    )
    ctx.actions.run(
        [
            "python3",
            script,
            ctx.attrs.yak_path,
            trace_out.as_output(),
        ],
        local_only = True,
        category = "trace",
    )

    nested_cmd = cmd_args(
        ctx.attrs.yak_path,
        "build",
        "root//:trivial",
        "-c",
        "nested.yak_path=" + ctx.attrs.yak_path,
        "--out",
        nested_out.as_output(),
        hidden = trace_out,
    )
    ctx.actions.run(
        nested_cmd,
        local_only = True,
        category = "run",
    )
    return [DefaultInfo(default_output = nested_out)]

normal_nested_invocation = rule(
    impl = _normal_impl,
    attrs = {
        "yak_path": attrs.string(),
    },
)

trace_nested_invocation = rule(
    impl = _trace_impl,
    attrs = {
        "yak_path": attrs.string(),
    },
)
