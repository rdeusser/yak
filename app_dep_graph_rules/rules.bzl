# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Avoid some copy-paste
def _app(s):
    return "//app/" + s + ":" + s

# These crates should only implement late bindings and not be depended on
# directly
LATE_BINDING_ONLY_CRATES = [
    _app("yak_anon_target"),
    _app("yak_cmd_audit_server"),
    _app("yak_cmd_query_server"),
    _app("yak_cmd_targets_server"),
    _app("yak_bxl"),
    _app("yak_query_impls"),
]

# These crates may only be depended on from `app/yak`
TOP_LEVEL_ONLY_CRATES = [
    _app("yak_cmd_debug_client"),
    _app("yak_cmd_log_client"),
]

# Unordered pairs where neither crate may depend on the other
BANNED_DEP_PATHS = [
    (_app("yak_common"), _app("yak_directory")),
    (_app("yak_common"), "//starlark-rust/starlark:starlark"),
    (_app("yak_build_api"), _app("yak_execute_impl")),
    (_app("yak_build_api"), _app("yak_interpreter_for_build")),
    (_app("yak_server"), _app("yak_server_commands")),
    (_app("yak_bxl"), _app("yak_configured")),
]
