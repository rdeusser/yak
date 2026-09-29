# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_keep_going_json(yak: Yak) -> None:
    result = await yak.targets("//...", "--json", "--keep-going")
    xs = json.loads(result.stdout)
    errors = [x for x in xs if "yak.error" in x]
    targets = sorted(x["name"] for x in xs if "name" in x)

    assert targets == [
        "target1",
        "target2",
        "target3",
        "target4",
        "target5",
        "target6",
    ]
    assert len(errors) == 1
    assert errors[0]["yak.package"] == "root//b"
    assert "test_error" in errors[0]["yak.error"]


@yak_test()
async def test_keep_going(yak: Yak) -> None:
    result = await yak.targets("//...", "--keep-going")
    assert "test_error" in result.stderr


@yak_test()
@pytest.mark.parametrize("no_cache", [False, True])
async def test_keep_going_streaming(yak: Yak, no_cache: bool) -> None:
    args = ["//...", "--json", "--streaming", "--keep-going"]
    if no_cache:
        args.append("--no-cache")
    result = await yak.targets(*args)
    xs = json.loads(result.stdout)
    errors = [x for x in xs if "yak.error" in x]
    targets = sorted(x["name"] for x in xs if "name" in x)

    assert targets == [
        "target1",
        "target2",
        "target3",
        "target4",
        "target5",
        "target6",
    ]
    assert len(errors) == 1
    assert errors[0]["yak.package"] == "root//b"
    assert "test_error" in errors[0]["yak.error"]


@yak_test()
async def test_streaming_keep_going_missing_targets(yak: Yak) -> None:
    targets = [
        "//a:target1",
        "//a:target2",
        "//a:bogus_target",
        "//a:worse_target",
        "//a:target5",
        "//d:bogus_package",
    ]
    result = await yak.targets(*targets, "--json", "--streaming", "--keep-going")
    xs = json.loads(result.stdout)
    assert len(xs) == 5  # 3 success, 2 errors
    bad_packages = []
    good_targets = []
    for x in xs:
        if "yak.error" in x:
            bad_packages.append(x["yak.package"])
            if x["yak.package"] == "root//a":
                assert "`bogus_target`" in x["yak.error"]
                assert "`worse_target`" in x["yak.error"]
        else:
            good_targets.append(x["name"])
    bad_packages.sort()
    good_targets.sort()
    assert bad_packages == ["root//a", "root//d"]
    assert good_targets == ["target1", "target2", "target5"]


@yak_test()
async def test_streaming_keep_going_with_single_failure(yak: Yak) -> None:
    targets = [
        "//a:does_not_exist",
    ]
    result = await yak.targets(*targets, "--json", "--streaming", "--keep-going")
    xs = json.loads(result.stdout)
    assert len(xs) == 1
    assert xs[0]["yak.package"] == "root//a"
    assert (
        xs[0]["yak.error"]
        == "Unknown targets `does_not_exist` from package `root//a`."
    )


@yak_test()
async def test_streaming_keep_going_with_single_failing_target_and_one_other_target_in_different_package(
    yak: Yak,
) -> None:
    targets = [
        "//a:target1",
        "//c:does_not_exist",
    ]
    result = await yak.targets(
        *targets,
        "-a",
        "type",
        "--streaming",
        "--keep-going",
    )

    xs = json.loads(result.stdout)
    assert len(xs) == 2

    if "yak.error" in xs[0]:
        good_target = xs[1]
        bad_target = xs[0]
    else:
        good_target = xs[0]
        bad_target = xs[1]

    assert good_target["yak.type"] == "prelude//prelude.bzl:a_target"

    assert bad_target["yak.package"] == "root//c"
    assert (
        bad_target["yak.error"]
        == "Unknown targets `does_not_exist` from package `root//c`."
    )
