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
async def test_output_artifact_twice_same(yak: Yak) -> None:
    res = await yak.build("root//:test_output_artifact_twice_same")
    assert (
        res.get_build_report()
        .output_for_target("root//:test_output_artifact_twice_same")
        .read_text()
        == "green lamp"
    )


@yak_test()
async def test_output_artifact_twice_with_projection(yak: Yak) -> None:
    res = await yak.build("root//:test_output_artifact_twice_with_projection")
    assert (
        res.get_build_report().output_for_target(
            "root//:test_output_artifact_twice_with_projection"
        )
        / "rel"
    ).read_text() == "red alert"
