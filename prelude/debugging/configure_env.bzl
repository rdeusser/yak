# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//debugging:labels.bzl", "DBG_INFO_DISABLE_INCOMPATIBLE_SANITIZERS")
load("@prelude//debugging:types.bzl", "UserMessage")

ConfiguredEnv = record(
    env = field(dict[str, str]),
    messages = field(list[UserMessage]),
)

_DISABLE_LEAK_DETECTION = "detect_leaks=0"
_ASAN_OPTIONS = "ASAN_OPTIONS"

def configure_env(env: dict[str, str], labels: list[str]) -> ConfiguredEnv:
    messages = []
    altered_env = {}
    if DBG_INFO_DISABLE_INCOMPATIBLE_SANITIZERS in labels:
        messages.append(
            UserMessage(
                title = "Leak sanitizer is disabled.",
                body = "You are trying to debug a binary with sanitizers. Certain sanitizers do not work under the debugger.\n",
            ),
        )
        altered_env[_ASAN_OPTIONS] = _inject_env(env, _ASAN_OPTIONS, _DISABLE_LEAK_DETECTION, lambda x: x + "," + _DISABLE_LEAK_DETECTION)

    # Return only the variables changed here, because BXL cannot expand macros such as
    # `$(exe //some:target)` in `env`. The caller takes the test's environment with macros
    # expanded from the test runner and applies these values over it.
    return ConfiguredEnv(env = altered_env, messages = messages)

def _inject_env(env: dict[str, str], key: str, value: str, merge: typing.Callable[[str], str]) -> str:
    existing_env = env.get(key)
    if existing_env:
        return merge(existing_env)
    else:
        return value
