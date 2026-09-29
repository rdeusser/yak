# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden, sanitize_stderr

# `within_view` is enforced while the build file is evaluated, so `uquery` is
# enough to exercise it (no configuration needed).

# Inside an opted-in subtree every refusal names both the target's own list and
# the cap; outside one, the message is the ordinary `within_view` error.
CAP_REGEX = (
    r"Target's effective `within_view` does not allow dependency"
    r".*Capped to.*enforce_within_view_intersection"
)
OWN_LIST_REGEX = r"Target's `within_view` attribute does not allow dependency"


@yak_test()
async def test_optin_inside_dep_is_allowed(yak: Yak) -> None:
    await yak.uquery("root//intersect/inside_dep:c")


@yak_test()
async def test_optin_cap_refuses_public_per_target_within_view(yak: Yak) -> None:
    # A per-target `within_view = ["PUBLIC"]` replaces the PACKAGE default but
    # cannot escape the cap. Locks the diagnostic: the target's own list and
    # the cap are both named.
    result = await expect_failure(
        yak.uquery("root//intersect/public_target:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_cap_refuses_public_per_target_within_view.golden.txt",
    )


@yak_test()
async def test_optin_cap_refuses_per_target_within_view_listing_outside_dep(
    yak: Yak,
) -> None:
    # The target's own `within_view` explicitly names `outside/...`; the cap
    # still refuses it.
    result = await expect_failure(
        yak.uquery("root//intersect/leaky_target:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_cap_refuses_per_target_within_view_listing_outside_dep.golden.txt",
    )


@yak_test()
async def test_per_target_within_view_narrower_than_cap_wins(yak: Yak) -> None:
    # The target's own list is what refuses the dep here (the dep is inside the
    # cap). Inside an opted-in subtree the message still names the cap next to
    # the target's own list, as `enforce_visibility_intersection()` does.
    result = await expect_failure(
        yak.uquery("root//intersect/narrow_target:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_per_target_within_view_narrower_than_cap_wins.golden.txt",
    )
    await yak.uquery("root//intersect/narrow_target_ok:c")


@yak_test()
async def test_nested_package_public_within_view_is_still_capped(
    yak: Yak,
) -> None:
    # `package(inherit = False, within_view = ["PUBLIC"])` in a descendant
    # replaces the default but not the cap.
    await yak.uquery("root//intersect/intermediate/leaf_ok:c")
    result = await expect_failure(
        yak.uquery("root//intersect/intermediate/leaf_bad:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_nested_package_public_within_view_is_still_capped.golden.txt",
    )


@yak_test()
async def test_intersect_grandparent_parent_child(yak: Yak) -> None:
    # Three opted-in levels: the effective cap is the AND of all three lists.
    await yak.uquery("root//intersect/child/grandchild/ok:c")
    result = await expect_failure(
        yak.uquery("root//intersect/child/grandchild/bad:c"),
        stderr_regex=r"Capped to .* AND .* AND .* by `enforce_within_view_intersection\(\)`",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_intersect_grandparent_parent_child.golden.txt",
    )


@yak_test()
async def test_optin_only_package_keeps_parent_default_and_cap(yak: Yak) -> None:
    # A PACKAGE that only calls `enforce_within_view_intersection()` (no
    # `package()`) neither widens the inherited `within_view` default nor
    # changes the cap. The default alone refuses the outside dep; the message
    # names the (inherited) cap as well.
    await yak.uquery("root//intersect/optin_only/ok:c")
    result = await expect_failure(
        yak.uquery("root//intersect/optin_only/bad:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_only_package_keeps_parent_default_and_cap.golden.txt",
    )


@yak_test()
async def test_optin_without_within_view_propagates_parent_cap(yak: Yak) -> None:
    # `package(inherit = True, visibility = [...])` (no `within_view=`) with
    # `enforce_within_view_intersection()` contributes nothing to the cap. The
    # parent's cap still applies -- and catches the PUBLIC default that
    # `inherit = True` with an omitted `within_view` produces.
    await yak.uquery("root//inherit_test/no_wv_child/inside_dep:c")
    result = await expect_failure(
        yak.uquery("root//inherit_test/no_wv_child/outside_dep:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_without_within_view_propagates_parent_cap.golden.txt",
    )


@yak_test()
async def test_inherit_true_child_can_still_tighten_cap(yak: Yak) -> None:
    # With `inherit = True`, the child contributes its EXPLICIT `within_view`
    # to the cap (not `parent ∪ child`), so the cap tightens even though the
    # default `within_view` below is the union.
    await yak.uquery("root//inherit_test/restricted_child/inside:c")
    result = await expect_failure(
        yak.uquery("root//inherit_test/restricted_child/escaping:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_inherit_true_child_can_still_tighten_cap.golden.txt",
    )


@yak_test()
async def test_public_is_identity(yak: Yak) -> None:
    await yak.uquery("root//public_identity/consumer:c")
    stdout = (await yak.audit("package-values", "root//public_identity")).stdout
    assert json.loads(stdout)["root//public_identity"]["within_view_cap"] == ["PUBLIC"]


@yak_test()
async def test_same_package_deps_are_exempt(yak: Yak) -> None:
    await yak.uquery("root//same_package:c")


@yak_test()
async def test_select_arm_is_capped(yak: Yak) -> None:
    result = await expect_failure(
        yak.uquery("root//select_arm:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_select_arm_is_capped.golden.txt",
    )


@yak_test()
async def test_exec_and_toolchain_deps_are_capped(yak: Yak) -> None:
    # `within_view` covers every dep-typed attribute, exec and toolchain deps
    # included; so does the cap.
    result = await expect_failure(
        yak.uquery("root//exec_dep:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_exec_dep_is_capped.golden.txt",
    )
    result = await expect_failure(
        yak.uquery("root//exec_dep/toolchain:c"),
        stderr_regex=CAP_REGEX,
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_toolchain_dep_is_capped.golden.txt",
    )


@yak_test()
async def test_outside_optin_message_is_unchanged(yak: Yak) -> None:
    # No ancestor opted in: the ordinary `within_view` error, without any
    # mention of a cap.
    result = await expect_failure(
        yak.uquery("root//no_optin/bad:c"),
        stderr_regex=OWN_LIST_REGEX,
    )
    assert "Capped to" not in result.stderr
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_outside_optin_message_is_unchanged.golden.txt",
    )


@yak_test()
async def test_audit_package_values_shows_within_view_cap(yak: Yak) -> None:
    stdout = (
        await yak.audit(
            "package-values",
            "root//intersect",
            "root//intersect/intermediate",
            "root//intersect/child/grandchild",
        )
    ).stdout
    result = json.loads(stdout)
    assert result["root//intersect"]["within_view_cap"] == ["root//intersect/..."]
    # A non-opted-in descendant inherits the cap unchanged, while its own
    # `within_view` is what its `package()` call said.
    assert result["root//intersect/intermediate"]["within_view_cap"] == [
        "root//intersect/..."
    ]
    assert result["root//intersect/intermediate"]["within_view"] == ["PUBLIC"]
    assert result["root//intersect/child/grandchild"]["within_view_cap"] == {
        "intersection": [
            ["root//intersect/..."],
            ["root//intersect/child/...", "root//outside/..."],
            [
                "root//intersect/child/grandchild/...",
                "root//intersect/child/lib:",
                "root//outside/...",
            ],
        ]
    }


@yak_test()
async def test_call_from_bzl_is_rejected(yak: Yak) -> None:
    result = await expect_failure(
        yak.uquery("root//indirect_call/leaf:t"),
        stderr_regex=r"`enforce_within_view_intersection\(\)` can only be called from a `PACKAGE` file",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_call_from_bzl_is_rejected.golden.txt",
    )


@yak_test()
async def test_calling_twice_in_one_package_is_rejected(yak: Yak) -> None:
    result = await expect_failure(
        yak.uquery("root//at_most_once/leaf:t"),
        stderr_regex=r"`enforce_within_view_intersection\(\)` function can be used at most once per `PACKAGE` file",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_calling_twice_in_one_package_is_rejected.golden.txt",
    )
