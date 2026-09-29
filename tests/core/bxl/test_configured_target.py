# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test(data_dir="")
async def test_unwrap_forward(yak: Yak) -> None:
    await yak.bxl("//bxl/configured_target.bxl:unwrap_forward")


@yak_test(data_dir="")
async def test_configured_targets_with_modifiers(yak: Yak) -> None:
    result = await yak.bxl(
        "//bxl/configured_target.bxl:configured_targets_with_modifiers"
    )
    configurations = [line.strip() for line in result.stdout.splitlines()]
    linux_cfg = await yak.audit_configurations(configurations[0])
    assert "root//:linux" in linux_cfg.stdout
    macos_cfg = await yak.audit_configurations(configurations[1])
    assert "root//:macos" in macos_cfg.stdout


@yak_test(data_dir="")
async def test_strip_cfg(yak: Yak) -> None:
    await yak.bxl("//bxl/configured_target.bxl:strip_cfg")
