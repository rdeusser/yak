# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


@yak_test()
async def test_visibility_from_package_simple(yak: Yak) -> None:
    result = await yak.uquery(
        "root//simple:", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="simple/golden.uquery.json",
    )


@yak_test()
async def test_visibility_from_package_inherit(yak: Yak) -> None:
    result = await yak.uquery(
        "root//inherit/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="inherit/golden.uquery.json",
    )


@yak_test()
async def test_visibility_from_package_override(yak: Yak) -> None:
    result = await yak.uquery(
        "root//override/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="override/golden.uquery.json",
    )


@yak_test()
async def test_visibility_from_package_public(yak: Yak) -> None:
    result = await yak.uquery(
        "root//public/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="public/golden.uquery.json",
    )
