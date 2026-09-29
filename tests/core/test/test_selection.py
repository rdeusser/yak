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
async def test_ok(yak: Yak) -> None:
    await yak.test("//:ok")


@yak_test()
async def test_fail(yak: Yak) -> None:
    await expect_failure(yak.test("//:fail"), stderr_regex="Fail: root//:fail")


@yak_test()
async def test_tests_attribute(yak: Yak) -> None:
    await expect_failure(
        yak.test("//:noop_references_fail"),
        stderr_regex="Fail: root//:fail",
    )


@yak_test()
async def test_tests_attribute_transitive(yak: Yak) -> None:
    await expect_failure(
        yak.test(
            "//:noop_transitively_references_fail",
        ),
        stderr_regex="Fail: root//:fail",
    )


@yak_test()
async def test_tests_attribute_cycle(yak: Yak) -> None:
    yak.test(
        "//:noop_cycle1",
    )


@yak_test()
async def test_tests_attribute_self_transition(yak: Yak) -> None:
    await expect_failure(
        yak.test("//:noop_self_transition_references_fail"),
        stderr_regex="Fail: root//:fail",
    )
