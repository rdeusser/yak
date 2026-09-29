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
async def test_bxl_caching(yak: Yak) -> None:
    result = await yak.bxl(
        "//caching.bxl:print_caching",
    )

    assert "ran me" in result.stderr
    assert "result print" in result.stdout

    result = await yak.bxl(
        "//caching.bxl:print_caching",
    )

    assert "ran me" not in result.stderr
    assert "result print" in result.stdout


@yak_test()
async def test_bxl_caching_with_target_platforms_specified(yak: Yak) -> None:
    # run with platform1, result should be cached afterwards
    result = await yak.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform1",
    )

    assert "ran me" in result.stderr
    assert "root//:platform1" in result.stdout

    # run with platform2, DICE should be invalidated and updated results should be
    # cached afterwards
    result = await yak.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform2",
    )

    assert "ran me" in result.stderr
    assert "root//:platform2" in result.stdout

    # run with platform1 again, we should already have cached results
    result = await yak.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform1",
    )

    assert "ran me" not in result.stderr
    assert "root//:platform1" in result.stdout


@yak_test()
async def test_bxl_error_caching(yak: Yak) -> None:
    result = await yak.bxl("//caching.bxl:print_error_caching")
    assert "ran me" in result.stderr
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:incompatible" in result.stderr

    # output stream that writes to stderr should be cached, but regular stdlib print
    # statements (which also write to stderr) will not be cached.
    result = await yak.bxl("//caching.bxl:print_error_caching")
    assert "ran me" not in result.stderr
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:incompatible" in result.stderr


@yak_test()
async def test_bxl_print_with_no_yakd(yak: Yak) -> None:
    result = await yak.bxl(
        "//caching.bxl:print_caching",
        "--no-yakd",
    )

    assert "ran me" in result.stderr
    assert "result print" in result.stdout
