# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import json_get, random_string


@yak_test()
async def test_local_action(yak: Yak) -> None:
    await yak.build(
        "//:foo",
        "--no-remote-cache",
        "--local-only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await yak.log("show")).stdout.strip().splitlines()

    for line in log:
        outputs = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "outputs",
        )
        if outputs is None:
            continue
        # e3b0c442 is the SHA-256 digest of an empty directory.
        # We have 2 directories "a" and "z", where
        # "a" is empty and "z" is not.
        # "z" is a first output for action.
        digests = [o["tiny_digest"] for o in outputs]
        assert len(digests) == 2
        # Checking that "a" is first in action outputs
        assert digests[0] == "e3b0c442"
        return

    raise AssertionError("Didn't find ActionExecution data")


@pytest.mark.remote_execution
@yak_test()
async def test_remote_action(yak: Yak) -> None:
    await yak.build(
        "//:foo",
        "--no-remote-cache",
        "--remote-only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await yak.log("show")).stdout.strip().splitlines()

    for line in log:
        outputs = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "outputs",
        )
        if outputs is None:
            continue
        # e3b0c442 is the SHA-256 digest of an empty directory.
        # We have 2 directories "a" and "z", where
        # "a" is empty and "z" is not.
        # "z" is a first output for action.
        digests = [o["tiny_digest"] for o in outputs]
        assert len(digests) == 2
        # Checking that "a" is first in action outputs
        assert digests[0] == "e3b0c442"
        return

    raise AssertionError("Didn't find ActionExecution data")
