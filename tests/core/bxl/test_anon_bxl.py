# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_define_anon_bxl(yak: Yak) -> None:
    await yak.bxl(
        "//anon_bxl.bxl:define_anon",
    )


@yak_test()
async def test_define_wrong_type_anon_bxl(yak: Yak) -> None:
    await expect_failure(
        yak.bxl("//wrong_type_anon_bxl.bxl:wrong_type"),
        stderr_regex="Type of parameter `impl` doesn't match,",
    )


@yak_test()
async def test_eval_anon_bxl(yak: Yak) -> None:
    await yak.bxl(
        "//anon_bxl.bxl:eval_anon_bxl",
    )


@yak_test()
async def test_check_anon_ouput_artifact(yak: Yak) -> None:
    await yak.bxl(
        "//anon_bxl.bxl:check_anon_ouput_artifact",
    )


@yak_test()
async def test_pass_string_to_arg_attr(yak: Yak) -> None:
    await yak.bxl("//anon_bxl.bxl:eval_of_anon_with_arg_bxl")


@yak_test()
async def test_content_based_output(yak: Yak) -> None:
    result = await yak.bxl(
        "//anon_bxl.bxl:eval_of_anon_with_content_based_output_impl"
    )

    output_path = (yak.cwd / result.stdout.strip()).resolve()
    assert output_path.read_text() == "hello world"
