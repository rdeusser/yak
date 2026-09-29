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
from e2e_util.helper.utils import random_string


@yak_test()
async def test_build_id_env_var_is_set_locally(yak: Yak) -> None:
    result = await yak.build(
        "root//:top",
        "--local-only",
        "--no-remote-cache",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    output = result.get_build_report().output_for_target("root//:top")
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == result.yak_build_id


@pytest.mark.remote_execution
@yak_test()
async def test_build_id_env_var_is_set_remotely(yak: Yak) -> None:
    result = await yak.build(
        "root//:top",
        "--remote-only",
        "--no-remote-cache",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    output = result.get_build_report().output_for_target("root//:top")
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == result.yak_build_id
