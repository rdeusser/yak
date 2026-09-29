# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os
import tempfile

from e2e_util.api.buck import Buck
from e2e_util.buck_workspace import buck_test
from e2e_util.helper.utils import filter_events


async def check_dice_equality(buck: Buck) -> None:
    dice_equal = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "DiceEqualityCheck",
        "is_equal",
    )
    assert len(dice_equal) == 1
    assert dice_equal[0] is True


async def check_config_is_the_same(buck: Buck) -> None:
    # We only fire this event where there are config invalidations.
    has_new_configs = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "CellHasNewConfigs",
    )
    assert len(has_new_configs) == 0


async def check_config_is_different(buck: Buck) -> None:
    # We only fire this event where there are config invalidations.
    has_new_configs = await filter_events(
        buck,
        "Event",
        "data",
        "Instant",
        "data",
        "CellHasNewConfigs",
    )
    assert len(has_new_configs) == 1

    assert has_new_configs[0]["cell"] == "root"


@buck_test()
async def test_ignore_state_invalidation_with_re_override_in_arg(buck: Buck) -> None:
    # Add arg to switch to yak-user
    await buck.build(
        "root//:simple",
        "--config",
        "yak_re_client.override_use_case=yak-user",
    )
    # No arg, default is yak-default
    await buck.build("root//:simple")
    await check_dice_equality(buck)
    await check_config_is_the_same(buck)
    # Add arg to switch to yak-user again
    await buck.build(
        "root//:simple",
        "--config",
        "yak_re_client.override_use_case=yak-user",
    )
    await check_dice_equality(buck)
    await check_config_is_the_same(buck)


@buck_test()
async def test_ignore_state_invalidation_with_re_override_in_config(buck: Buck) -> None:
    # Default is yak-default
    await buck.build("root//:simple")
    # Add config to switch to yak-user
    with open(buck.cwd / ".yakconfig.local", "w") as f:
        f.write("[yak_re_client]\n")
        f.write("override_use_case = yak-user\n")
    await buck.build("root//:simple")
    await check_config_is_different(buck)
    # Add config to return to yak-default
    with open(buck.cwd / ".yakconfig.local", "w") as f:
        f.write("[yak_re_client]\n")
        f.write("override_use_case = yak-default\n")
    await buck.build("root//:simple")
    await check_config_is_different(buck)


@buck_test()
async def test_ignore_state_invalidation_with_re_override_in_external_config(
    buck: Buck,
) -> None:
    # Default is yak-default
    await buck.build("root//:simple")
    # Add config to switch to yak-user
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[yak_re_client]\n")
        f.write("override_use_case = yak-user\n")
        f.close()
        await buck.build("root//:simple", "--config-file", f.name)
    await check_config_is_different(buck)
    # Add config to return to yak-default
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[yak_re_client]\n")
        f.write("override_use_case = yak-default\n")
        f.close()
        await buck.build("root//:simple", "--config-file", f.name)
    await check_config_is_different(buck)


@buck_test()
async def test_ignore_state_invalidation_with_re_override_in_external_config_source(
    buck: Buck,
) -> None:
    with tempfile.NamedTemporaryFile("w", delete=False) as temp:
        env = os.environ.copy()
        env["YAK_TEST_EXTRA_EXTERNAL_CONFIG"] = temp.name

        # Default is yak-default
        await buck.build("root//:simple", env=env)

        # Add config to switch to yak-user
        temp.write("[yak_re_client]\n")
        temp.write("override_use_case = yak-user\n")
        temp.flush()
        await buck.build("root//:simple", env=env)
        await check_config_is_different(buck)

        # Add config to return to yak-default
        temp.seek(0)
        temp.truncate()
        temp.write("[yak_re_client]\n")
        temp.write("override_use_case = yak-default\n")
        temp.flush()
        await buck.build("root//:simple", env=env)
        await check_dice_equality(buck)
