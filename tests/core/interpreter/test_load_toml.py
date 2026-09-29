# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import ast
from typing import Any

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def extract_test_output(stderr: str) -> dict[str, Any]:
    """Extract the DATA_LOAD_TEST_OUTPUT dict from yak stderr."""
    marker = "DATA_LOAD_TEST_OUTPUT: "
    for line in stderr.splitlines():
        idx = line.find(marker)
        if idx != -1:
            return ast.literal_eval(line[idx + len(marker) :])  # type: ignore[no-any-return]
    raise AssertionError(f"Could not find '{marker}' in stderr:\n{stderr}")


def assert_common_types(types: dict[str, Any]) -> None:
    """Assertions for types shared between JSON and TOML."""
    assert types["string_val"] == "hello"
    assert types["int_val"] == 42
    assert types["negative_int"] == -7
    assert types["float_val"] == 3.14
    assert types["negative_float"] == -0.5
    assert types["bool_true"] is True
    assert types["bool_false"] is False
    assert types["string_array"] == ["a", "b", "c"]
    assert types["int_array"] == [1, 2, 3]
    assert types["bool_array"] == [True, False, True]
    assert types["nested_table"] == {"key": "value", "number": 99}


def assert_common_fruits(data: dict[str, Any]) -> None:
    """Assertions for the fruits array-of-objects structure."""
    fruits = data["fruits"]
    assert len(fruits) == 2
    assert fruits[0]["name"] == "apple"
    assert fruits[0]["physical"] == {"color": "red", "shape": "round"}
    assert fruits[1]["name"] == "banana"


@yak_test()
async def test_load_toml(yak: Yak) -> None:
    result = await yak.targets("root//:")
    data = extract_test_output(result.stderr)

    assert_common_types(data["types"])
    assert_common_fruits(data)

    # TOML-specific: datetime is converted to a string
    assert isinstance(data["types"]["datetime_val"], str)
    assert "2026" in data["types"]["datetime_val"]
