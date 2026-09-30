# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

_ATTRS = {
    "deps": attrs.list(attrs.dep(), default = []),
    "srcs": attrs.list(attrs.source(), default = []),
}

lib = rule(impl = lambda _ctx: [DefaultInfo()], attrs = _ATTRS)

# Each test fails, so that yak prints the label of every test it runs.
def failing_test_impl(_ctx):
    return [
        DefaultInfo(),
        ExternalRunnerTestInfo(
            command = ["python3", "-c", "import sys; sys.exit(1)"],
            type = "custom",
        ),
    ]

check = rule(impl = failing_test_impl, attrs = _ATTRS)
