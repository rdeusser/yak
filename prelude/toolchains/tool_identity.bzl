# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def tool_identity_attr():
    """
    tool_identity_attr returns the `tool_identity` attribute of a toolchain that
    runs a tool from `PATH`.
    """
    return attrs.string(
        default = "",
        doc = """
            The identity of the tool that the toolchain runs from `PATH`, which goes into the key
            of every action that runs it, so that the actions run again when the tool changes.
            `system_toolchains()` passes `read_root_config("tool_identity", <tool>, "")`, which the
            daemon computes from the tool's version output at the start of each command.
        """,
    )
