# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _long_running_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("out", has_content_based_path = False)
    ctx.actions.run(
        ["python3", "-c", "import time, sys; time.sleep(999999); open(sys.argv[1],'w')", out.as_output()],
        category = "test",
        identifier = "id",
    )

    return [DefaultInfo(out)]

long_running = rule(
    impl = _long_running_impl,
    attrs = {},
)

# The action starts a child, writes "<action pid> <child pid>" to the file that
# `-c test.pid_file=<path>` names, and sleeps.
_SPAWNS_CHILD = """
import os, subprocess, sys, time
child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(999999)"])
with open(sys.argv[1] + ".tmp", "w") as f:
    f.write(f"{os.getpid()} {child.pid}")
os.rename(sys.argv[1] + ".tmp", sys.argv[1])
time.sleep(999999)
"""

def _spawns_child_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("out", has_content_based_path = False)
    ctx.actions.run(
        ["python3", "-c", _SPAWNS_CHILD, read_root_config("test", "pid_file"), out.as_output()],
        category = "test",
        identifier = "spawns_child",
    )

    return [DefaultInfo(out)]

spawns_child = rule(
    impl = _spawns_child_impl,
    attrs = {},
)
