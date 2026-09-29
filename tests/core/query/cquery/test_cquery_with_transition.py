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


@yak_test()
async def test_cquery_transition_without_target_universe(yak: Yak) -> None:
    result = await yak.cquery(
        "root//:yak",
        "--target-platforms=root//:p",
    )

    # Both configurations for the target are returned: the default, and the transition
    lines = result.stdout.splitlines()
    assert 2 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"

    # Test cquery with "%s".
    result = await yak.cquery(
        "%s",
        "root//:yak",
        "root//:moose",
        "--target-platforms=root//:p",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-elk#<HASH>)"

    # Test cquery with "%Ss"
    result = await yak.cquery(
        "%Ss",
        "root//:yak",
        "root//:moose",
        "--target-platforms=root//:p",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-elk#<HASH>)"


@yak_test()
async def test_cquery_transition_with_target_universe(yak: Yak) -> None:
    result = await yak.cquery(
        "root//:yak",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:yak",
    )

    lines = result.stdout.splitlines()
    assert 2 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"

    # Test cquery with "%s".
    result = await yak.cquery(
        "%s",
        "root//:yak",
        "root//:moose",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:yak,root//:moose",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-elk#<HASH>)"

    # Test cquery with "%Ss".
    result = await yak.cquery(
        "%Ss",
        "root//:yak",
        "root//:moose",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:yak,root//:moose",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:yak (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:yak (transitioned-to-elk#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-elk#<HASH>)"
