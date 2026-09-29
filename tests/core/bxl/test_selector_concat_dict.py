# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_not_selector_attr(yak: Yak) -> None:
    await yak.bxl(
        "//:selector_concat_dict.bxl:not_selector_attr",
    )


@yak_test()
async def test_selector_dict_attr(yak: Yak) -> None:
    await yak.bxl(
        "//:selector_concat_dict.bxl:selector_dict_attr",
    )


@yak_test()
async def test_selector_concat_attr(yak: Yak) -> None:
    await yak.bxl(
        "//:selector_concat_dict.bxl:selector_concat_attr",
    )


@yak_test()
async def test_selector_dict_write_json(yak: Yak) -> None:
    res = await yak.bxl(
        "//:selector_concat_dict.bxl:selector_dict_write_json",
    )
    file_path = res.stdout.strip()
    with open(file_path, "r") as f:
        content = f.read()
    expected_content = {
        "__type": "selector",
        "entries": {
            "DEFAULT": ["--foo", "--bar"],
            "root//constraints:macos": ["--foo-macos", "--bar-macos"],
            "root//constraints:x86": ["--foo-x86", "--bar-x86"],
        },
    }
    assert json.loads(content) == expected_content


@yak_test()
async def test_selector_concat_write_json(yak: Yak) -> None:
    res = await yak.bxl(
        "//:selector_concat_dict.bxl:selector_concat_write_json",
    )
    file_path = res.stdout.strip()
    with open(file_path, "r") as f:
        content = f.read()
    expected_content = {
        "__type": "concat",
        "items": [
            ["--flag", "--baz"],
            {
                "__type": "selector",
                "entries": {
                    "DEFAULT": ["--foo", "--bar"],
                    "root//constraints:macos": ["--foo-macos", "--bar-macos"],
                    "root//constraints:x86": ["--foo-x86", "--bar-x86"],
                },
            },
        ],
    }
    assert json.loads(content) == expected_content
