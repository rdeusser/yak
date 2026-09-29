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


def _targets(stdout: str) -> set[str]:
    return {line.strip() for line in stdout.splitlines() if line.strip()}


@yak_test()
async def test_uquery_deps_without_allow_partial_graph_fails(yak: Yak) -> None:
    # `root//:root` depends on `//broken:b`, whose package fails to parse, so a
    # plain `deps` traversal aborts.
    await expect_failure(
        yak.uquery("deps(root//:root)"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_uquery_deps_allow_partial_graph_skips_broken_edge(yak: Yak) -> None:
    # The `deps` traversal is open-ended, so a broken dependency edge discovered
    # while searching is skipped; the loadable part is returned.
    result = await yak.uquery("deps(root//:root)", "--allow-partial-graph")
    targets = _targets(result.stdout)
    assert "root//:root" in targets
    assert "root//good:g" in targets
    assert not any(t.startswith("root//broken:") for t in targets)


@yak_test()
async def test_uquery_rdeps_without_allow_partial_graph_fails(yak: Yak) -> None:
    # The universe `root//...` contains the broken package, so enumerating it
    # aborts the reverse query.
    await expect_failure(
        yak.uquery("rdeps(root//..., root//good:g)"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_uquery_rdeps_allow_partial_graph_skips_broken_package(
    yak: Yak,
) -> None:
    # `root//...` is open-ended, so the broken package under the universe is
    # skipped instead of aborting. The explicit `root//good:g` argument resolves
    # fine.
    result = await yak.uquery(
        "rdeps(root//..., root//good:g)", "--allow-partial-graph"
    )
    targets = _targets(result.stdout)
    # `root` and `good_only` both depend on `good:g`.
    assert "root//good:g" in targets
    assert "root//:root" in targets
    assert "root//:good_only" in targets
    assert not any(t.startswith("root//broken:") for t in targets)


@yak_test()
async def test_uquery_recursive_allow_partial_graph_skips_broken_package(
    yak: Yak,
) -> None:
    # Without the flag, enumerating the recursive pattern aborts on the broken
    # package.
    await expect_failure(
        yak.uquery("root//..."),
        stderr_regex="intentional parse error",
    )

    # With the flag, the open-ended `root//...` enumeration skips the broken
    # package and returns everything else.
    result = await yak.uquery("root//...", "--allow-partial-graph")
    targets = _targets(result.stdout)
    assert "root//:root" in targets
    assert "root//:leaf" in targets
    assert "root//:good_only" in targets
    assert "root//good:g" in targets
    assert not any(t.startswith("root//broken:") for t in targets)


@yak_test()
async def test_uquery_allow_partial_graph_explicit_missing_target_still_fails(
    yak: Yak,
) -> None:
    # An explicitly named missing target must resolve; `--allow-partial-graph`
    # only tolerates breakage in open-ended (recursive) patterns, so this fails
    # with or without the flag.
    await expect_failure(
        yak.uquery("root//:root + root//:nonexistent"),
        stderr_regex="Unknown target `nonexistent`",
    )
    await expect_failure(
        yak.uquery("root//:root + root//:nonexistent", "--allow-partial-graph"),
        stderr_regex="Unknown target `nonexistent`",
    )


@yak_test()
async def test_uquery_allow_partial_graph_explicit_broken_package_still_fails(
    yak: Yak,
) -> None:
    # An explicitly named target in a broken package must resolve, so this fails
    # with or without the flag.
    await expect_failure(
        yak.uquery("root//broken:b"),
        stderr_regex="intentional parse error",
    )
    await expect_failure(
        yak.uquery("root//broken:b", "--allow-partial-graph"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_uquery_rdeps_explicit_broken_arg_still_fails(yak: Yak) -> None:
    # The universe `root//...` is open-ended and tolerated, but the explicit
    # `root//broken:b` argument names a broken package, so the query still fails
    # even with `--allow-partial-graph`.
    await expect_failure(
        yak.uquery("rdeps(root//..., root//broken:b)", "--allow-partial-graph"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_uquery_rdeps_explicit_broken_universe_still_fails(yak: Yak) -> None:
    # Contrast with the open-ended `root//...` universe: naming a broken package
    # explicitly as the universe fails even with `--allow-partial-graph`.
    await expect_failure(
        yak.uquery("rdeps(root//broken:b, root//good:g)", "--allow-partial-graph"),
        stderr_regex="intentional parse error",
    )


@yak_test()
async def test_uquery_allow_partial_graph_clean_query_unaffected(yak: Yak) -> None:
    # `--allow-partial-graph` on an error-free query returns the normal result.
    result = await yak.uquery("deps(root//good:g)", "--allow-partial-graph")
    assert _targets(result.stdout) == {"root//good:g"}
