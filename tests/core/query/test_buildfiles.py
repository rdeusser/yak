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
async def test_allbuildfiles(yak: Yak) -> None:
    target1 = "root//load:abc"
    target2 = "root//transitive_load:def"
    target3 = "root//transitive_load:ghi"
    out1 = (await yak.uquery(f"allbuildfiles({target1})")).stdout
    out2 = (await yak.uquery(f"allbuildfiles({target2})")).stdout
    out3 = (await yak.uquery(f"allbuildfiles({target3})")).stdout
    out4 = (await yak.uquery(f"allbuildfiles(set({target1} {target2}))")).stdout

    # First, check that these are the same for cquery
    assert out1 == (await yak.cquery(f"allbuildfiles({target1})")).stdout
    assert out2 == (await yak.cquery(f"allbuildfiles({target2})")).stdout
    assert out3 == (await yak.cquery(f"allbuildfiles({target3})")).stdout
    assert (
        out4 == (await yak.cquery(f"allbuildfiles(set({target1} {target2}))")).stdout
    )

    out1 = [x for x in out1.splitlines() if not x.startswith("nano_prelude/")]
    out1.sort()
    out2 = [x for x in out2.splitlines() if not x.startswith("nano_prelude/")]
    out2.sort()
    out3 = [x for x in out3.splitlines() if not x.startswith("nano_prelude/")]
    out3.sort()
    out4 = [x for x in out4.splitlines() if not x.startswith("nano_prelude/")]
    out4.sort()

    # verify loads
    expected1 = ["load/YAK.fixture", "load/a.bzl", "load/a.json"]
    assert out1 == expected1

    # verify transitive loads
    expected2 = [
        "transitive_load/YAK.fixture",
        "transitive_load/b.bzl",
        "transitive_load/c.bzl",
        "transitive_load/c.json",
    ]
    assert out2 == expected2
    assert out3 == expected2

    # correctly handle multiple inputs
    expected4 = expected1 + expected2
    expected4.sort()
    assert out4 == expected4


@yak_test()
async def test_rbuildfiles(yak: Yak) -> None:
    target_file = "transitive_load/YAK.fixture"
    out1 = (
        await yak.uquery(f"rbuildfiles({target_file}, transitive_load/c.bzl)")
    ).stdout
    out2 = (await yak.uquery(f"rbuildfiles({target_file}, {target_file})")).stdout

    # Check that these are the same for cquery
    assert (
        out1
        == (
            await yak.cquery(f"rbuildfiles({target_file}, transitive_load/c.bzl)")
        ).stdout
    )
    assert (
        out2 == (await yak.cquery(f"rbuildfiles({target_file}, {target_file})")).stdout
    )

    assert "transitive_load/b.bzl" in out1
    assert "transitive_load/c.bzl" in out1
    assert "transitive_load/YAK" in out1

    assert out2 == target_file + "\n"
