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
from e2e_util.helper.golden import golden, sanitize_stderr


@yak_test(allow_soft_errors=True)
async def test_unified_constraint_defination(yak: Yak) -> None:
    await yak.bxl("//test_unified_constraint.bxl:main")


@yak_test()
async def test_unified_constraint_miss_default_fail(yak: Yak) -> None:
    await expect_failure(
        yak.targets("//miss_default:"),
        stderr_regex=".*Missing named-only parameter `default` for call to `constraint`.*",
    )


@yak_test()
async def test_unified_constraint_default_not_appear_in_value_fail(yak: Yak) -> None:
    await expect_failure(
        yak.audit("subtargets", "//default_value_not_appear:"),
        stderr_regex=r""".*default value 'linux' must be one of the declared values: \["macos", "windows"\].*""",
    )


# 'default' as a value is allowed only when it is also the default value (see the `compiler`
# constraint in the root fixture). Here it is a value but NOT the default, so it must fail as ambiguous.
# Distinct test names are used because test discovery treats 'default' and 'DEFAULT' as the same name
# (case-insensitive).
@yak_test()
async def test_unified_constraint_reserved_keyword_default_lowercase_fail(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.audit("subtargets", "//reserved_keyword_default_lowercase:"),
        stderr_regex=".*'default' can be used as a constraint value only when it is also the default value.*",
    )


# Test reserved keyword 'DEFAULT' (uppercase)
@yak_test()
async def test_unified_constraint_reserved_keyword_default_uppercase_fail(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.audit("subtargets", "//reserved_keyword_default_uppercase:"),
        stderr_regex=".*'DEFAULT' is a reserved keyword and cannot be used as a constraint value.*",
    )


@yak_test(allow_soft_errors=True)
async def test_unified_constraint_cfg_transition(yak: Yak) -> None:
    await yak.bxl("//test_unified_constraint.bxl:test_cfg_transition")


@yak_test(allow_soft_errors=True)
async def test_unified_constraint_cfg_transition_v2(yak: Yak) -> None:
    await yak.bxl("//test_unified_constraint.bxl:test_cfg_transition_v2")


@yak_test(allow_soft_errors=True)
async def test_unified_constraint_for_constraint_v2(yak: Yak) -> None:
    await yak.bxl("//test_unified_constraint.bxl:constraint_v2")


@yak_test()
async def test_unified_constraint_single_value_without_flag_fail(
    yak: Yak,
) -> None:
    res = await expect_failure(
        yak.audit("subtargets", "//single_value_no_flag:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/single_value_no_flag.golden.stderr",
    )


@yak_test()
async def test_unified_constraint_single_value_with_flag(yak: Yak) -> None:
    await yak.audit("subtargets", "//single_value_with_flag:")


@yak_test()
async def test_unified_constraint_zero_values_with_flag_fail(yak: Yak) -> None:
    res = await expect_failure(
        yak.audit("subtargets", "//zero_values_with_flag:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/zero_values_with_flag.golden.stderr",
    )


@yak_test()
async def test_unified_constraint_alias_conflict_with_value_fail(
    yak: Yak,
) -> None:
    res = await expect_failure(
        yak.audit("subtargets", "//alias_conflict_with_value:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_conflict_with_value.golden.stderr",
    )


@yak_test()
async def test_unified_constraint_alias_value_not_declared_fail(
    yak: Yak,
) -> None:
    res = await expect_failure(
        yak.audit("subtargets", "//alias_value_not_declared:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_value_not_declared.golden.stderr",
    )


@yak_test()
async def test_unified_constraint_alias_reserved_keyword_fail(
    yak: Yak,
) -> None:
    res = await expect_failure(
        yak.audit("subtargets", "//alias_reserved_keyword:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_reserved_keyword.golden.stderr",
    )
