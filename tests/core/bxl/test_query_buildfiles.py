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
async def test_uquery_allbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:uquery_allbuildfiles",
    )


@yak_test()
async def test_uquery_rbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:uquery_rbuildfiles",
    )


@yak_test()
async def test_cquery_allbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:cquery_allbuildfiles",
    )


@yak_test()
async def test_cquery_rbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:cquery_rbuildfiles",
    )


@yak_test()
async def test_lazy_uquery_allbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:lazy_uquery_allbuildfiles",
    )


@yak_test()
async def test_lazy_uquery_rbuildfiles(yak: Yak) -> None:
    await yak.bxl(
        "//:query_buildfiles.bxl:lazy_uquery_rbuildfiles",
    )
