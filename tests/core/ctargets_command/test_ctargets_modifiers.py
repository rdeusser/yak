# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


def _extract_configuration(s: str) -> list[str]:
    return re.findall(r"\((.*?)\)", s)


@yak_test()
async def test_ctargets_modifier_single_pattern(yak: Yak) -> None:
    result = await yak.ctargets("root//:target?root//:macos")

    [configuration] = _extract_configuration(result.stdout)

    macos_cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in macos_cfg.stdout


@yak_test()
async def test_ctargets_modifier_multiple_patterns(yak: Yak) -> None:
    result = await yak.ctargets(
        "root//:target?root//:macos", "root//:other_target?root//:macos"
    )

    [target_configuration, other_configuration] = _extract_configuration(result.stdout)

    target_cfg = await yak.audit_configurations(target_configuration)
    assert "root//:macos" in target_cfg.stdout

    other_cfg = await yak.audit_configurations(other_configuration)
    assert "root//:macos" in other_cfg.stdout


@yak_test()
async def test_ctargets_modifier_multiple_modifiers(yak: Yak) -> None:
    result = await yak.ctargets("root//:target?root//:macos+root//:arm")

    [configuration] = _extract_configuration(result.stdout)

    multi_cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in multi_cfg.stdout
    assert "root//:arm" in multi_cfg.stdout


@yak_test()
async def test_ctargets_modifier_order_of_modifiers(yak: Yak) -> None:
    # if passing in modifiers of the same constraint setting,
    # the last one should be the one that applies
    result = await yak.ctargets("root//:target?root//:macos+root//:linux")

    [configuration] = _extract_configuration(result.stdout)

    cfg = await yak.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout
    assert "root//:macos" not in cfg.stdout


@yak_test()
async def test_ctargets_modifier_multi_target_pattern(yak: Yak) -> None:
    result = await yak.ctargets("root//:?root//:macos")

    [other_configuration, _, configuration] = _extract_configuration(result.stdout)[5:]

    other_cfg = await yak.audit_configurations(other_configuration)
    assert "root//:macos" in other_cfg.stdout

    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_ctargets_modifier_same_target(yak: Yak) -> None:
    # if the same target has the same modifiers there should only be one instance of it
    result = await yak.ctargets(
        "root//:target?root//:macos", "root//:target?root//:macos"
    )

    [configuration] = _extract_configuration(result.stdout)

    cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_ctargets_fails_with_global_modifier(yak: Yak) -> None:
    await expect_failure(
        yak.ctargets("--modifier", "root//:linux", "root//:target?root//:macos"),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )
