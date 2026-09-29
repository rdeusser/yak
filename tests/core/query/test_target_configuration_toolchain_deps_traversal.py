# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden, golden_replace_cfg_hash


@yak_test()
# Test `target_deps()` function does not include toolchain deps.
async def test_cquery_target_deps(yak: Yak) -> None:
    result = await yak.cquery("deps(tests/..., 1, target_deps())")
    # TODO(nga): this test does not test that any target deps are actually returned.
    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="cquery_target_deps.golden",
    )


@yak_test()
# Test `target_deps()` function does not include toolchain deps.
async def test_uquery_target_deps(yak: Yak) -> None:
    # TODO(nga): output includes `platform_windows` target, which is probably not meant to be there.
    result = await yak.uquery("deps(tests/..., 1, target_deps())")
    golden(
        output=result.stdout,
        rel_path="uquery_target_deps.golden",
    )


# Test `configuration_deps()` function does include configuration deps.
@yak_test()
async def test_cquery_configuration_deps(yak: Yak) -> None:
    q = "deps(tests/..., 1, configuration_deps())"
    result = await yak.cquery(q)
    # Note test output includes `root//tests:python_only`, which is not a configuration deps.
    # This is now `deps()` with traversal function works: it includes roots.
    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="cquery_configuration_deps.golden",
    )


# Test `configuration_deps()` function does include configuration deps.
@yak_test()
async def test_uquery_configuration_deps(yak: Yak) -> None:
    q = "deps(tests/..., 1, configuration_deps())"
    result = await yak.uquery(q)
    # TODO(nga): this does not return any configuration deps.
    golden(
        output=result.stdout,
        rel_path="uquery_configuration_deps.golden",
    )


@yak_test()
async def test_cquery_toolchain_deps(yak: Yak) -> None:
    q = "deps(tests:python_and_asic, 1, toolchain_deps())"
    out = await yak.cquery(q)
    golden_replace_cfg_hash(
        output=out.stdout,
        rel_path="cquery_toolchain_deps.golden",
    )


@yak_test()
async def test_uquery_toolchain_deps(yak: Yak) -> None:
    q = "deps(tests:python_and_asic, 1, toolchain_deps())"
    out = await yak.uquery(q)
    golden(
        output=out.stdout,
        rel_path="uquery_toolchain_deps.golden",
    )
