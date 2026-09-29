# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakResult
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events, random_string


# Incremental actions use the output of previous actions, mimic this behavior by
# appending a string - Note that this is not how incremental actions behave in practice
async def basic_incremental_action_local_only_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"
    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar bar"


@yak_test()
async def test_basic_incremental_action_local_only(yak: Yak) -> None:
    await basic_incremental_action_local_only_helper(yak, use_content_based_path=False)


@yak_test()
async def test_basic_incremental_action_local_only_with_content_based_path(
    yak: Yak,
) -> None:
    await basic_incremental_action_local_only_helper(yak, use_content_based_path=True)


async def incremental_action_from_remote_action_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "--remote-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"


@pytest.mark.remote_execution
@yak_test()
async def test_incremental_action_from_remote_action(yak: Yak) -> None:
    await incremental_action_from_remote_action_helper(
        yak, use_content_based_path=False
    )


@pytest.mark.remote_execution
@yak_test()
async def test_incremental_action_from_remote_action_with_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_from_remote_action_helper(
        yak, use_content_based_path=True
    )


async def incremental_action_with_non_incremental_remote_action_inbetween_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--remote-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        "test.use_incremental=false",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"


@pytest.mark.remote_execution
@yak_test()
async def test_incremental_action_with_non_incremental_remote_action_inbetween(
    yak: Yak,
) -> None:
    await incremental_action_with_non_incremental_remote_action_inbetween_helper(
        yak, use_content_based_path=False
    )


@pytest.mark.remote_execution
@yak_test()
async def test_incremental_action_with_non_incremental_remote_action_inbetween_with_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_with_non_incremental_remote_action_inbetween_helper(
        yak, use_content_based_path=True
    )


async def incremental_action_with_non_incremental_local_action_inbetween_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        "test.use_incremental=false",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_with_non_incremental_local_action_inbetween(
    yak: Yak,
) -> None:
    await incremental_action_with_non_incremental_local_action_inbetween_helper(
        yak, use_content_based_path=False
    )


@yak_test()
async def test_incremental_action_with_non_incremental_local_action_inbetween_with_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_with_non_incremental_local_action_inbetween_helper(
        yak, use_content_based_path=True
    )


async def basic_incremental_action_cached_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "--remote-only",
    )
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    # This is the correct/expected behavior because it means that the cached output was used and the action was
    # not re-executed because re-execution would have resulted in the output to be "foo bar". See below
    assert result.stdout == "foo"

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"


@pytest.mark.remote_execution
@yak_test()
async def test_basic_incremental_action_cached(yak: Yak) -> None:
    await basic_incremental_action_cached_helper(yak, use_content_based_path=False)


@pytest.mark.remote_execution
@yak_test()
async def test_basic_incremental_action_cached_with_content_based_path(
    yak: Yak,
) -> None:
    await basic_incremental_action_cached_helper(yak, use_content_based_path=True)


async def basic_incremental_action_after_cache_hit_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    # Populate the remote cache
    result = await yak.run(
        "root//:basic_incremental_action",
        "--remote-only",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    await yak.clean()

    # Run again, and make sure we got an action cache hit
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    execution_kinds = await filter_events(
        yak,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )
    ACTION_EXECUTION_KIND_ACTION_CACHE = 3
    assert execution_kinds[-1] == ACTION_EXECUTION_KIND_ACTION_CACHE

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )

    assert result.stdout == "foo bar"


@pytest.mark.remote_execution
@yak_test()
async def test_basic_incremental_action_after_cache_hit(yak: Yak) -> None:
    await basic_incremental_action_after_cache_hit_helper(
        yak, use_content_based_path=False
    )


@pytest.mark.remote_execution
@yak_test()
async def test_basic_incremental_action_after_cache_hit_with_content_based_path(
    yak: Yak,
) -> None:
    await basic_incremental_action_after_cache_hit_helper(
        yak, use_content_based_path=True
    )


async def incremental_action_interleave_platforms_helper(
    yak: Yak, platform: str, use_content_based_path: bool
) -> YakResult:
    return await yak.run(
        "root//:basic_incremental_action",
        "--target-platforms",
        platform,
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )


@yak_test()
async def test_incremental_action_interleave_platforms_aabb(yak: Yak) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_different_platforms_abab(yak: Yak) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_different_platforms_abba(yak: Yak) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=False
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=False
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_interleave_platforms_aabb_with_content_based_path(
    yak: Yak,
) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_interleave_platforms_abab_with_content_based_path(
    yak: Yak,
) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_interleave_platforms_abba_with_content_based_path(
    yak: Yak,
) -> None:
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_cat", use_content_based_path=True
    )
    assert result.stdout == "foo bar"
    result = await incremental_action_interleave_platforms_helper(
        yak, "root//:p_default", use_content_based_path=True
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_with_metadata_opt_out(
    yak: Yak,
) -> None:
    await yak.build("root//:incremental_action_with_metadata_optout")


# We shouldn't lose the state from killing the daemon in between invocations
async def incremental_action_persist_between_daemon_restart_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    await yak.kill()

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"


@yak_test()
async def test_incremental_action_persist_between_daemon_restart(
    yak: Yak,
) -> None:
    await incremental_action_persist_between_daemon_restart_helper(
        yak, use_content_based_path=False
    )


@yak_test()
async def test_incremental_action_persist_between_daemon_restart_with_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_persist_between_daemon_restart_helper(
        yak, use_content_based_path=True
    )


# If we haven't materialized the outputs, then we won't run incrementally on the first run
# after a daemon restart
async def unmaterialized_incremental_action_not_persist_between_daemon_restart_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    await yak.build(
        "root//:basic_incremental_action",
        "--remote-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
        "--materializations",
        "none",
    )

    await yak.kill()

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"


@pytest.mark.remote_execution
@yak_test()
async def test_unmaterialized_incremental_action_not_persist_between_daemon_restart(
    yak: Yak,
) -> None:
    await unmaterialized_incremental_action_not_persist_between_daemon_restart_helper(
        yak, use_content_based_path=False
    )


@pytest.mark.remote_execution
@yak_test()
async def test_unmaterialized_incremental_action_not_persist_between_daemon_restart_with_content_based_path(
    yak: Yak,
) -> None:
    await unmaterialized_incremental_action_not_persist_between_daemon_restart_helper(
        yak, use_content_based_path=True
    )


# Clean wipes yak-out, which should reset everything so incremental actions should start anew
async def incremental_action_clean_resets_state_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    await yak.clean()

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"


@yak_test()
async def test_incremental_action_clean_resets_state(
    yak: Yak,
) -> None:
    await incremental_action_clean_resets_state_helper(
        yak, use_content_based_path=False
    )


@yak_test()
async def test_incremental_action_clean_resets_state_with_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_clean_resets_state_helper(
        yak, use_content_based_path=True
    )


# In practice, there will be multiple actions with multiple outputs running. This test
# mimics that behavior a bit to ensure the states don't step over each other.
async def incremental_action_multi_outputs_with_daemon_restart_helper(
    yak: Yak, use_content_based_path: bool
) -> None:
    result = await yak.run(
        "root//:basic_incremental_action",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo"

    await yak.kill()

    result = await yak.run(
        "root//:incremental_action_with_multiple_outputs",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "ab"

    await yak.kill()

    result = await yak.run(
        "root//:basic_incremental_action",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "foo bar"

    await yak.kill()

    result = await yak.run(
        "root//:incremental_action_with_multiple_outputs",
        "--local-only",
        "-c",
        f"test.seed={random_string()}",
        "-c",
        f"test.use_content_based_path={use_content_based_path}",
    )
    assert result.stdout == "aabb"


@yak_test()
async def test_incremental_action_multi_outputs_with_daemon_restart(
    yak: Yak,
) -> None:
    await incremental_action_multi_outputs_with_daemon_restart_helper(
        yak, use_content_based_path=False
    )


@yak_test()
async def test_incremental_action_multi_outputs_with_daemon_restart_and_content_based_path(
    yak: Yak,
) -> None:
    await incremental_action_multi_outputs_with_daemon_restart_helper(
        yak, use_content_based_path=True
    )


@yak_test(
    extra_yak_config={"yak": {"sqlite_incremental_state": "false"}},
)
async def test_incremental_action_db_disabled(
    yak: Yak,
) -> None:
    await basic_incremental_action_local_only_helper(yak, use_content_based_path=True)
