# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import platform
import random
import string
from pathlib import Path

import pytest
from e2e_util.api.yak import Yak
from e2e_util.api.yak_result import YakException
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import json_get, read_what_ran


@yak_test(data_dir="anon_exec_deps")
async def test_anon_target_exec_deps(yak: Yak) -> None:
    await yak.build("//tests:exec_dep_good", "--remote-only")

    await expect_failure(
        yak.build("//tests:exec_dep_bad", "--local-only"),
        stderr_regex="Exec deps and the current anon target must have the same execution platform resolution",
    )

    await expect_failure(
        yak.build("//tests:exec_dep_rejects_dep"),
        stderr_regex="exec dep is missing the execution platform resolution",
    )


@yak_test(data_dir="args")
async def test_args(yak: Yak) -> None:
    result = await yak.build("//:bin")
    output = result.get_build_report().output_for_target("//:bin")
    assert (
        output.read_text().rstrip()
        == "<foo_compiler> -- <foo_compiler_flags>\nlib1 -- this is lib1\nlib2 -- this is lib2"
    )


@yak_test(data_dir="prelude_import")
async def test_prelude_imported_once(yak: Yak) -> None:
    # See the comments in the relevant targets files: they explain how this
    # test works.
    await yak.build("cell1//...", "cell2//...")


def read_all_outputs(yak: Yak, report: str) -> list[str]:
    ret = []

    with open(yak.cwd / report) as f:
        report = json.load(f)
        for _target, state in report["results"].items():
            for configured in state["configured"].values():
                ret.extend(configured["outputs"].get("DEFAULT", []))
                ret.extend(configured["other_outputs"].get("DEFAULT", []))

    return ret


@yak_test(data_dir="build_providers")
async def test_build_providers(yak: Yak) -> None:
    await yak.build(
        "//:target",
        "--build-default-info",
        "--skip-run-info",
        "--skip-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(yak, "report")
    assert any("/build" in o for o in outputs)
    assert all("/run" not in o for o in outputs)
    assert all("/test" not in o for o in outputs)

    await yak.build(
        "//:target",
        "--skip-default-info",
        "--build-run-info",
        "--skip-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(yak, "report")
    assert all("/build" not in o for o in outputs)
    assert all("/test" not in o for o in outputs)

    await yak.build(
        "//:target",
        "--skip-default-info",
        "--skip-run-info",
        "--build-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(yak, "report")
    assert all("/build" not in o for o in outputs)
    assert all("/run" not in o for o in outputs)


@yak_test(data_dir="projected_artifacts")
@pytest.mark.parametrize(
    "target",
    [
        # Check building the whole thing
        "//...",
        # Check building just one target, which may reveal bugs if things are
        # materialized differently when a projected target uses them.
        "//:check_c_b_local",
    ],
)
async def test_projected_artifacts(yak: Yak, target: str) -> None:
    await yak.build(target)


@yak_test(data_dir="yakroot")
async def test_yakroot(yak: Yak) -> None:
    # Test that .yakroot files work
    await yak.build(":inner", rel_cwd=Path("rooted/cell"))


@yak_test(data_dir="cell_delete")
async def test_cell_deletion(yak: Yak) -> None:
    """
    This is a regression test for https://github.com/facebook/yak/pull/43,
    including the similar issue with directories that was fixed first.
    """
    await yak.targets(":")
    (yak.cwd / "hello").mkdir()
    await yak.targets(":")
    (yak.cwd / "hello").rmdir()
    await yak.targets(":")


@pytest.mark.xfail(
    reason="the fs_hash_crawler file watcher that tests use fails the command on a file name that contains a backslash",
    strict=True,
)
@yak_test(
    data_dir="invalid_file_invalidation",
    skip_for_os=["windows"],
)
async def test_invalid_file_invalidation(yak: Yak) -> None:
    """
    Checks that files and directories with invalid names do not break later builds.
    """

    await yak.build(":root")

    src = yak.cwd / "src"
    invalid = src / "\\"
    invalid_nested = src / "\\" / "a"
    invalid_nested_invalid = src / "\\" / "\\"

    # Create an invalid file. Build should work.
    invalid.touch()
    output = await yak.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # Delete it, build should work.
    invalid.unlink()
    await yak.build(":root")

    # Create an invalid dir. Build should still work.
    invalid_nested.mkdir(parents=True)
    output = await yak.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # And delete it. Things should work.
    invalid_nested.rmdir()
    invalid.rmdir()
    await yak.build(":root")

    # Finally, do an invalid file inside an invalid dir...
    invalid_nested_invalid.mkdir(parents=True)
    output = await yak.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # And delete it. Things should again.
    invalid_nested_invalid.rmdir()
    invalid.rmdir()
    await yak.build(":root")


@yak_test(data_dir="concurrency")
async def test_concurrency(yak: Yak) -> None:
    await yak.build("//:weight", "--local-only", "--no-remote-cache")

    # Now, since our commands request 20% of resources, check that a no point
    # we had more than 5 running commands. Also check that we found the right
    # amount of commands.
    log = (await yak.log("show")).stdout.strip().splitlines()

    running_execs = {}
    execs_done = 0

    for line in log:
        id = json_get(line, "Event", "span_id")

        is_end = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
        )

        if is_end:
            if running_execs.pop(id, None) is not None:
                execs_done += 1

            continue

        is_local_exec = json_get(
            line,
            "Event",
            "data",
            "SpanStart",
            "data",
            "ExecutorStage",
            "stage",
            "Local",
            "stage",
            "Execute",
        )

        if is_local_exec:
            running_execs[id] = True

        # Check that concurrently running local commands
        # don't exceed 5.
        assert len(running_execs) <= 5

    assert execs_done == 10


@yak_test(data_dir="fail_fast")
async def test_fail_fast(yak: Yak) -> None:
    with pytest.raises(YakException) as exc:
        await yak.build(
            "root//:mixed",
            "root//:slow",
            "--local-only",
            "--no-remote-cache",
        )

    assert "fast_default_output" in exc.value.stderr
    assert "slow_default_output" in exc.value.stderr
    assert "slow_other_output" in exc.value.stderr

    with pytest.raises(YakException) as exc:
        await yak.build(
            "root//:mixed",
            "root//:slow",
            "--local-only",
            "--no-remote-cache",
            "--fail-fast",
        )

    assert "fast_default_output" in exc.value.stderr
    assert "slow_default_output" not in exc.value.stderr
    assert "slow_other_output" not in exc.value.stderr


@yak_test(data_dir="keep_going_build")
async def test_keep_going(yak: Yak) -> None:
    with pytest.raises(YakException) as exc:
        await yak.build(
            "root//:top",
            "--local-only",
            "--no-remote-cache",
        )

    assert "fast_action" in exc.value.stderr
    assert "slow_action" not in exc.value.stderr

    # Dont want to re-attach to the ongoing evaluation for slow_action.
    # Normally that gets cancelled, but even so that's still a race.
    await yak.kill()

    with pytest.raises(YakException) as exc:
        await yak.build(
            "root//:top", "--local-only", "--no-remote-cache", "--keep-going"
        )

    assert "fast_action" in exc.value.stderr
    assert "slow_action" in exc.value.stderr


@yak_test(data_dir="cleanup")
async def test_cleanup(yak: Yak) -> None:
    # Checks that yak cleans up outputs whose parent directories became files.
    target_pattern = "//:cleanup"
    result = await yak.build(target_pattern)
    output = result.get_build_report().output_for_target(target_pattern)

    # The output should be something like path/__cleanup__/out/dir1/dir2/output.txt
    # We want to ensure that if we make a file dir1 or dir1/dir2, cleanup still works
    output.unlink()
    output.parent.rmdir()
    output.parent.write_text("File that must be deleted")
    await yak.kill()
    await yak.build(target_pattern)

    output.unlink()
    output.parent.rmdir()
    output.parent.parent.rmdir()
    output.parent.parent.write_text("File that must be deleted")
    await yak.build(target_pattern)


@pytest.mark.remote_execution
@yak_test(data_dir="log_action_keys")
async def test_log_action_keys(yak: Yak) -> None:
    async def read_action_keys() -> list[tuple[str, str]]:
        out = await read_what_ran(yak)
        return [
            (
                line["reproducer"]["executor"],
                line["reproducer"]["details"]["action_key"],
            )
            for line in out
        ]

    seed = random_string()
    action_key = "executor root//:test (<unspecified>) touch"

    # Run on RE
    await yak.build(
        ":test", "-c", f"test.seed={seed}", "-c", "yak.log_action_keys=true"
    )
    assert await read_action_keys() == [("Re", action_key)]

    await yak.kill()

    # Run on RE again, get a cache hit this time
    await yak.build(
        ":test", "-c", f"test.seed={seed}", "-c", "yak.log_action_keys=true"
    )

    assert await read_action_keys() == [("Cache", action_key)]


@yak_test(data_dir="roots")
async def test_roots(yak: Yak) -> None:
    res = await yak.build("root//:test", "other//:test")

    is_windows: bool = platform.system() == "Windows"

    def platformify(path: str) -> str:
        if is_windows:
            return path.replace("/", "\\")
        return path

    output = res.get_build_report().output_for_target("root//:test")
    with open(output) as f:
        j = json.load(f)
        print(j)
        assert (yak.cwd / j["fixture_relative_to_cell"]).exists()
        assert (yak.cwd / j["fixture_relative_to_project"]).exists()

        assert j["cell_relative_to_fixture"] == platformify("../../../../../../..")
        assert j["project_relative_to_fixture"] == platformify("../../../../../../..")

    output = res.get_build_report().output_for_target("other//:test")
    with open(output) as f:
        j = json.load(f)
        assert (yak.cwd / "other" / j["fixture_relative_to_cell"]).exists()
        assert (yak.cwd / j["fixture_relative_to_project"]).exists()

        assert j["cell_relative_to_fixture"] == platformify(
            "../../../../../../../other"
        )
        assert j["project_relative_to_fixture"] == platformify("../../../../../../..")


@yak_test(data_dir="tmpdir")
async def test_tmpdir(yak: Yak) -> None:
    await yak.build("root//:")


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))


@yak_test(data_dir="artifact_consistency")
async def test_artifact_consistency(yak: Yak) -> None:
    out = await yak.build_without_report(
        ":gen[file3]",
        "--local-only",
        "--out=-",
    )

    assert out.stdout == "This is file3"

    out = await yak.build_without_report(
        "-c",
        "gen.idx=2",
        ":gen[file3]",
        "--local-only",
        "--out=-",
    )
    assert out.stdout == "This is file3"
