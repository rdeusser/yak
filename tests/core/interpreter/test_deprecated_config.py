# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
@pytest.mark.parametrize("section", ["some", "other"])
@pytest.mark.parametrize("root", ["true", "false"])
async def test_deprecated_config(yak: Yak, section: str, root: str) -> None:
    _ = await expect_failure(
        yak.build(
            f":test_target_{section}_config1",
            "-c",
            f"test.section={section}",
            "-c",
            "test.conf=config1",
            "-c",
            f"test.root={root}",
        ),
        stderr_regex=f"{section}.config1 is no longer used. Please use other.config2",
    )


@yak_test()
@pytest.mark.parametrize("root", ["true", "false"])
async def test_not_deprecated_config(yak: Yak, root: str) -> None:
    section = "other"
    _ = await yak.build(
        f":test_target_{section}_config2",
        "-c",
        f"test.section={section}",
        "-c",
        "test.conf=config2",
        "-c",
        f"test.root={root}",
    )


@yak_test()
async def test_no_deprecated_cell_config(yak: Yak) -> None:
    section = "other"
    await yak.build(
        f"cell//:test_target_{section}_config1",
        "-c",
        f"test.section={section}",
        "-c",
        "test.conf=config1",
    )


@yak_test()
async def test_deprecated_cell_config2(yak: Yak) -> None:
    section = "other"
    _ = await expect_failure(
        yak.build(
            f"cell//:test_target_{section}_config2",
            "-c",
            f"test.section={section}",
            "-c",
            "test.conf=config2",
        ),
        stderr_regex=f"{section}.config2 is no longer used. Please use other.config3",
    )
