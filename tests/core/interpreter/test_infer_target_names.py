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
from e2e_util.helper.golden import golden

# Turns `//lib/greeting` into `//lib/greeting:greeting` during coercion in
# build/bzl files. On by default.
_DISABLE = "yak.infer_target_names=false"


@yak_test()
async def test_eponymous_dep_in_build_file_rejected_when_disabled(yak: Yak) -> None:
    # `//:consumer` depends on the abbreviated `//lib/greeting`. With inference
    # off this is a coercion error.
    res = await expect_failure(
        yak.uquery("deps(root//:consumer)", "--console=none", "-v0", "-c", _DISABLE),
    )
    golden(
        output=res.stderr,
        rel_path="golden/eponymous_dep_rejected.golden.stderr",
    )


@yak_test()
async def test_eponymous_dep_in_build_file_inferred_by_default(yak: Yak) -> None:
    # `//lib/greeting` is inferred to be `//lib/greeting:greeting`, which is a
    # real target, so the query succeeds.
    res = await yak.uquery("deps(root//:consumer)")
    assert "root//lib/greeting:greeting" in res.stdout


@yak_test()
async def test_eponymous_source_in_build_file_rejected_when_disabled(yak: Yak) -> None:
    # `attrs.source()` takes either a path or a label, so it coerces separately
    # from `attrs.dep()`. With inference off, `//lib/greeting` fails as both a
    # label without an explicit target name and as a non-relative path.
    await expect_failure(
        yak.uquery(
            "deps(root//source:src_consumer)", "--console=none", "-v0", "-c", _DISABLE
        ),
        stderr_regex=r"Couldn't coerce `//lib/greeting` as a source",
    )


@yak_test()
async def test_eponymous_source_in_build_file_inferred_by_default(
    yak: Yak,
) -> None:
    # The source attr coerces to the eponymous label rather than being
    # (mis)treated as a path.
    res = await yak.uquery("deps(root//source:src_consumer)")
    assert "root//lib/greeting:greeting" in res.stdout


@yak_test()
async def test_eponymous_attr_default_in_bzl_rejected_when_disabled(yak: Yak) -> None:
    # The `//bzl_default` rule declares `attrs.dep(default = "//lib/greeting")`.
    # The default is coerced while evaluating the .bzl module, so it errors
    # with inference off.
    res = await expect_failure(
        yak.uquery("root//bzl_default:", "--console=none", "-v0", "-c", _DISABLE),
    )
    golden(
        output=res.stderr,
        rel_path="golden/eponymous_attr_default_rejected.golden.stderr",
    )


@yak_test()
async def test_eponymous_attr_default_in_bzl_inferred_by_default(
    yak: Yak,
) -> None:
    # The eponymous attr default resolves and shows up as a dependency of the
    # target.
    res = await yak.uquery("deps(root//bzl_default:target)")
    assert "root//lib/greeting:greeting" in res.stdout
