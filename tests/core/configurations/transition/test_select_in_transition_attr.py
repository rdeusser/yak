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
async def test_transition_success_if_attr_value_has_not_changed(yak: Yak) -> None:
    await yak.build("root//:target_where_transition_does_not_change_attr")


@yak_test()
async def test_transition_dep_success_if_attr_value_has_not_changed(yak: Yak) -> None:
    await yak.build("root//:target_with_transition_dep")


@yak_test()
async def test_transition_failed_if_attr_value_has_changed(yak: Yak) -> None:
    err_msg = (
        r"Target root//:target_where_transition_changes_attr configuration transitioned\n"
        r"\s+old: root//:iphone#.*\n"
        r"\s+new: <transitioned-from-watch>#.*\n"
        r"\s+but attribute: extra\n"
        r"\s+resolved with old configuration to: \"phone\"\n"
        r"\s+resolved with new configuration to: \"watch\""
    )

    await expect_failure(
        yak.build("root//:target_where_transition_changes_attr"),
        stderr_regex=err_msg,
    )


@yak_test()
async def test_transition_failed_if_attr_value_cycle(yak: Yak) -> None:
    err_msg = (
        r"Configured target cycle detected \(`->` means \"depends on\"\):\n"
        r"\s+root//:target_where_transition_cycles_via_changed_attrs \(<transitioned-from-.*>#.*\) ->.*\n"
        r"\s+root//:target_where_transition_cycles_via_changed_attrs \(<transitioned-from-.*>#.*\) ->.*\n"
    )

    await expect_failure(
        yak.build("root//:target_where_transition_cycles_via_changed_attrs"),
        stderr_regex=err_msg,
    )
