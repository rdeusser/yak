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
async def test_projected_output_artifact_write(yak: Yak) -> None:
    res = await yak.build("root//:write")
    # TODO(nga): this is a bug: we write into projected artifact, but return original artifact,
    #   and yet here we read from original non-projected artifact.
    assert (
        "ccoonntteenntt"
        == res.get_build_report().output_for_target("root//:write").read_text()
    )


@yak_test()
async def test_projected_output_artifact_run(yak: Yak) -> None:
    res = await yak.build("root//:run")
    assert (
        "hello"
        == (res.get_build_report().output_for_target("root//:run") / "rel").read_text()
    )
