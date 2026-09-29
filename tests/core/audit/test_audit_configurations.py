# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


def _parse_audit_configurations(output: str) -> list[str]:
    return [x.rstrip(":") for x in output.splitlines() if not x.startswith(" ")]


@yak_test()
async def test_audit_configurations_all(yak: Yak) -> None:
    # Evaluate a target to make sure configuration is loaded.
    await yak.cquery("//:genrule")

    result = await yak.audit("configurations")
    configurations = _parse_audit_configurations(result.stdout)
    configurations = [_replace_hash(x) for x in configurations]
    assert "root//:p#<HASH>" in configurations


@yak_test()
async def test_audit_configurations_specific(yak: Yak) -> None:
    # Evaluate a target to make sure configuration is loaded.
    await yak.cquery("//:genrule")

    # Load configurations so we can learn the hash.
    result = await yak.audit("configurations")
    configurations = _parse_audit_configurations(result.stdout)
    [configuration] = [c for c in configurations if c.startswith("root//:p#")]

    # Now audit the specific configuration.
    result = await yak.audit("configurations", configuration)
    assert [configuration] == _parse_audit_configurations(result.stdout)
