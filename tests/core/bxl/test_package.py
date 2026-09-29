# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_get_package_path(yak: Yak) -> None:
    await yak.bxl(
        "//package.bxl:get_package_path",
    )


@yak_test()
async def test_read_package_value(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_value")


@yak_test()
async def test_read_package_value_from_string(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_value_from_string")


@yak_test()
async def test_read_override_package_value(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_override_package_value")


@yak_test()
async def test_read_package_value_not_found(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_value_not_found")


@yak_test()
async def test_read_package_visibility(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_visibility")


@yak_test()
async def test_read_package_within_view(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_within_view")


@yak_test()
async def test_read_package_visibility_cap(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_visibility_cap")


@yak_test()
async def test_read_package_within_view_cap(yak: Yak) -> None:
    await yak.bxl("//package.bxl:read_package_within_view_cap")
