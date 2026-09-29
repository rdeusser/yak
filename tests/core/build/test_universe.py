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
async def test_build_universe(yak: Yak) -> None:
    # Run the build without universe.
    result = await yak.build("//:test")
    build_report = result.get_build_report()
    output = build_report.output_for_target("//:test")
    assert output.read_text().rstrip() == "default"

    # Now build the same target, but with the universe.
    result = await yak.build(
        "//:test",
        "--target-universe",
        "//:universe",
    )
    build_report = result.get_build_report()
    output = build_report.output_for_target("//:test")
    assert output.read_text().rstrip() == "cat"


@yak_test()
async def test_build_target_not_found_in_universe(yak: Yak) -> None:
    result = await yak.build(
        "//:test",
        "--target-universe",
        "//:different_universe",
    )

    assert (
        "No targets found inside the specified universe, nothing will be built"
        in result.stderr
    )
