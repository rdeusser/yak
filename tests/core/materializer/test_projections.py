# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
A projection is a subpath of another target's directory artifact. These tests
consume projections from local actions, which forces their inputs onto disk, and
check that what those actions read is the current content: when one directory
is consumed at two granularities, when the projected subpath or the directory's
shape changes between builds, and across a daemon restart.
"""

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import replace_in_file


async def read_consumer(yak: Yak, target: str) -> str:
    """Build `target` and return what its action read from its inputs."""
    result = await yak.build(target)
    return result.get_build_report().output_for_target(target).read_text()


@yak_test()
async def test_two_projections_of_one_artifact(yak: Yak) -> None:
    result = await yak.build("root//:split_a", "root//:split_b")
    report = result.get_build_report()

    assert report.output_for_target("root//:split_a").read_text() == "split-a"
    assert report.output_for_target("root//:split_b").read_text() == "split-b"


@yak_test()
async def test_projection_and_whole_artifact_in_one_build(yak: Yak) -> None:
    result = await yak.build("root//:mixed_projection", "root//:mixed_whole")
    report = result.get_build_report()

    assert report.output_for_target("root//:mixed_projection").read_text() == "mixed-a"
    assert (
        report.output_for_target("root//:mixed_whole").read_text() == "mixed-a|mixed-b"
    )


@yak_test()
async def test_projected_subpath_changes_between_builds(yak: Yak) -> None:
    target = "root//:moving_consumer"
    targets_file = yak.cwd / "YAK.fixture"

    assert await read_consumer(yak, target) == "moving-a-1"

    # Both the projected subpath and the whole directory's contents change, so
    # neither the old subpath's bytes nor the old bytes at the new subpath are
    # an acceptable answer.
    replace_in_file(
        'consume(name = "moving_consumer", dep = ":moving", projections = ["a"])',
        'consume(name = "moving_consumer", dep = ":moving", projections = ["b"])',
        targets_file,
    )
    replace_in_file("moving-a-1", "moving-a-2", targets_file)
    replace_in_file("moving-b-1", "moving-b-2", targets_file)

    assert await read_consumer(yak, target) == "moving-b-2"


@pytest.mark.remote_execution
@yak_test()
async def test_projection_consumed_after_daemon_restart(yak: Yak) -> None:
    # Declare the directory artifact without putting it on disk, so that the
    # work of materializing it falls to the restarted daemon.
    result = await yak.build("root//:restart", "--materializations=None")
    produced = result.get_build_report().output_for_target("root//:restart")
    assert not produced.exists()

    await yak.kill()

    assert await read_consumer(yak, "root//:restart_consumer") == "restart-a"
    assert produced.exists()


@yak_test()
async def test_whole_artifact_after_projection_at_newer_contents(yak: Yak) -> None:
    targets_file = yak.cwd / "YAK.fixture"
    whole = "root//:interleaved_whole"
    projection = "root//:interleaved_projection"

    assert await read_consumer(yak, whole) == "interleaved-a-1|interleaved-b-1"

    replace_in_file("interleaved-a-1", "interleaved-a-2", targets_file)
    replace_in_file("interleaved-b-1", "interleaved-b-2", targets_file)

    # Only the projection is consumed at the new contents, so this build need
    # not bring `b` up to date on disk...
    assert await read_consumer(yak, projection) == "interleaved-a-2"
    # ...but the whole-directory consumer must not then see a current `a` next
    # to a stale `b`.
    assert await read_consumer(yak, whole) == "interleaved-a-2|interleaved-b-2"


@yak_test()
async def test_projected_subpath_did_not_exist_before(yak: Yak) -> None:
    target = "root//:growing_consumer"
    targets_file = yak.cwd / "YAK.fixture"

    assert await read_consumer(yak, target) == "growing-a"

    replace_in_file(
        'contents = {"a": "growing-a"}',
        'contents = {"a": "growing-a", "b": "growing-b"}',
        targets_file,
    )
    replace_in_file(
        'consume(name = "growing_consumer", dep = ":growing", projections = ["a"])',
        'consume(name = "growing_consumer", dep = ":growing", projections = ["b"])',
        targets_file,
    )

    assert await read_consumer(yak, target) == "growing-b"
