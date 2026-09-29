# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

# TODO(iguridi) or TODO(raulgarcia4):
# New `audit` commands have been added since these tests were created.
# Test them if necessary.


@yak_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    [
        "audit_visibility",
        "audit_configurations",
        "audit_config",
        "audit_visibility",
    ],
)
async def test_pass_common_opts_func(yak: Yak, cmd: str) -> None:
    cmd_call = getattr(yak, cmd)
    await cmd_call("--client-metadata", "id=placeholder_id")


@yak_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    [
        "analysis-queries",
        "cell",
        "execution-platform-resolution",
        "includes",
        "prelude",
        "providers",
        "subtargets",
    ],
)
async def test_pass_common_opts(yak: Yak, cmd: str) -> None:
    commands_requiring_target_pattern_arg_value = {"providers", "subtargets"}

    if cmd in commands_requiring_target_pattern_arg_value:
        await yak.audit(cmd, "//:dummy", "--client-metadata", "id=placeholder_id")
    else:
        await yak.audit(cmd, "--client-metadata", "id=placeholder_id")
