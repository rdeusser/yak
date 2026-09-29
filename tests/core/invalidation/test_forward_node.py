# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


@yak_test()
async def test_forward_node_supports_cutoff(yak: Yak) -> None:
    await yak.targets("--show-output", "root//:main")
    # Add a file to the root directory
    with open(yak.cwd / "YAK.fixture", "a") as targetsfile:
        targetsfile.write("\n# a comment\n")
    await yak.targets("--show-output", "root//:main")

    events = await filter_events(yak, "Event", "data", "SpanEnd", "data")
    loads = []
    analyses = []

    for ev in events:
        if "Load" in ev:
            loads.append(ev)
        if "Analysis" in ev:
            analyses.append(ev)

    assert len(loads) > 0
    # TODO(cjhopman): fix
    assert len(analyses) == 0, "should not have analysed anything"
