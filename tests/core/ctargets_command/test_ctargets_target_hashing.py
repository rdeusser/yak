# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from __future__ import annotations

import json
import re

import pytest
from yak.tests.e2e_util.api.yak import Yak
from yak.tests.e2e_util.asserts import expect_failure
from yak.tests.e2e_util.yak_workspace import yak_test
from yak.tests.e2e_util.helper.golden import golden_replace_cfg_hash


_HASH_REGEX: re.Pattern[str] = re.compile(r"^(?:[0-9a-f]{32}|[0-9a-f]{64})$")


async def _target_hash(yak: Yak, target: str, *args: str) -> str:
    result = await yak.ctargets(
        target,
        "--show-target-hash",
        "--json",
        *args,
    )
    output = json.loads(result.stdout)
    assert len(output) == 1, output
    target_hash = output[0]["yak.target_hash"]
    assert _HASH_REGEX.fullmatch(target_hash), target_hash
    return target_hash


@pytest.mark.parametrize("output_format", ["json", "text"])
@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_hash_output_golden(
    yak: Yak, output_format: str, hash_function: str, recursive: bool
) -> None:
    args = ["--json"] if output_format == "json" else []
    if recursive:
        args.append("--target-hash-recursive")
    result = await yak.ctargets(
        "root//:parent?root//:linux",
        "--show-target-hash",
        f"--target-hash-function={hash_function}",
        *args,
    )
    golden_replace_cfg_hash(
        output=re.sub(
            r"\b(?:[0-9a-f]{32}|[0-9a-f]{64})\b", "<TARGET_HASH>", result.stdout
        ),
        rel_path=f"golden/hash.{output_format}.golden",
    )


@yak_test()
async def test_hash_ignores_configuration_but_tracks_configured_attrs(
    yak: Yak,
) -> None:
    same_result = await yak.ctargets(
        "root//:same?root//:linux",
        "root//:same?root//:macos",
        "--show-target-hash",
        "--json",
    )
    same_outputs = json.loads(same_result.stdout)
    assert len(same_outputs) == 2, same_outputs

    selected_result = await yak.ctargets(
        "root//:selected?root//:linux",
        "root//:selected?root//:macos",
        "--show-target-hash",
        "--json",
    )
    selected_outputs = json.loads(selected_result.stdout)
    assert len(selected_outputs) == 2, selected_outputs

    assert same_outputs[0]["yak.target_hash"] == same_outputs[1]["yak.target_hash"]
    assert (
        selected_outputs[0]["yak.target_hash"]
        != selected_outputs[1]["yak.target_hash"]
    )


@pytest.mark.parametrize("output_format", ["text", "json", "json-report"])
@yak_test()
async def test_hash_deduplicates_targets_after_transition(
    yak: Yak, output_format: str
) -> None:
    args = [] if output_format == "text" else [f"--{output_format}"]
    result = await yak.ctargets(
        "root//:transitioned?root//:linux",
        "root//:transitioned?root//:macos",
        "--show-target-hash",
        *args,
    )
    if output_format == "text":
        [line] = result.stdout.splitlines()
        assert line.startswith("root//:transitioned (")
        assert _HASH_REGEX.fullmatch(line.rsplit(" ", 1)[1])
        return

    targets = json.loads(result.stdout)
    if output_format == "json-report":
        targets = targets["compatible_targets"]
    [target] = targets
    assert target["yak.target"] == "root//:transitioned"
    assert target["yak.type"] == "root//defs.bzl:transitioned"
    assert _HASH_REGEX.fullmatch(target["yak.target_hash"])


@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_hash_ignores_forward_dependency_nodes(
    yak: Yak, hash_function: str, recursive: bool
) -> None:
    target = "root//:transition_parent"
    with_forward_dep = f"{target}?root//:linux"
    args = [f"--target-hash-function={hash_function}"]
    if recursive:
        args.append("--target-hash-recursive")

    baseline = await _target_hash(yak, target, *args)
    assert baseline == await _target_hash(yak, with_forward_dep, *args)

    changed_args = (*args, "-c", "test.transitioned_value=after")
    changed = await _target_hash(yak, target, *changed_args)
    assert (baseline != changed) == recursive
    assert changed == await _target_hash(yak, with_forward_dep, *changed_args)

    forced_args = (*args, "--require-hash-change-deps", "root//:transitioned")
    forced = await _target_hash(yak, target, *forced_args)
    assert baseline != forced
    assert forced == await _target_hash(yak, with_forward_dep, *forced_args)


@yak_test()
async def test_hash_is_available_in_all_output_formats(yak: Yak) -> None:
    target = "root//:same?root//:linux"

    plain_json = json.loads((await yak.ctargets(target, "--json")).stdout)
    assert "yak.target_hash" not in plain_json[0]

    text = (await yak.ctargets(target, "--show-target-hash")).stdout
    assert re.search(r" [0-9a-f]{32}\n$", text), text

    report = json.loads(
        (
            await yak.ctargets(
                target,
                "--show-target-hash",
                "--json-report",
            )
        ).stdout
    )
    assert _HASH_REGEX.fullmatch(report["compatible_targets"][0]["yak.target_hash"])


@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_hash_tracks_dependency_attributes_only(
    yak: Yak, recursive: bool
) -> None:
    args = ("--target-hash-recursive",) if recursive else ()
    baseline = await _target_hash(yak, "root//:parent?root//:linux", *args)

    targets_file = yak.cwd / "TARGETS.fixture"
    targets_file.write_text(
        targets_file.read_text().replace("unrelated-before", "unrelated-after")
    )
    await yak.kill()
    unrelated_changed = await _target_hash(yak, "root//:parent?root//:linux", *args)
    assert baseline == unrelated_changed

    targets_file.write_text(targets_file.read_text().replace("dep-before", "dep-after"))
    await yak.kill()
    dependency_changed = await _target_hash(yak, "root//:parent?root//:linux", *args)
    assert (baseline != dependency_changed) == recursive


@yak_test()
async def test_hash_does_not_read_source_contents(yak: Yak) -> None:
    baseline = await _target_hash(yak, "root//:with_src?root//:linux")

    (yak.cwd / "source.txt").write_text("changed contents\n")
    await yak.kill()

    assert baseline == await _target_hash(yak, "root//:with_src?root//:linux")
    assert baseline != await _target_hash(
        yak,
        "root//:with_src?root//:linux",
        "--require-hash-change-deps",
        "root//:with_src",
    )


@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_require_hash_change_deps(
    yak: Yak, hash_function: str, recursive: bool
) -> None:
    targets = [
        "dep",
        "parent",
        "grandparent",
        "great_grandparent",
        "with_src",
        "unrelated",
    ]
    args = [f"--target-hash-function={hash_function}", "--show-target-hash", "--json"]
    if recursive:
        args.append("--target-hash-recursive")
    args.extend(f"root//:{target}?root//:linux" for target in targets)
    baseline = json.loads((await yak.ctargets(*args)).stdout)
    changed = json.loads(
        (
            await yak.ctargets(
                *args, "--require-hash-change-deps", "root//:dep", ":with_src"
            )
        ).stdout
    )
    expected = {"root//:dep", "root//:parent", "root//:with_src"}
    if recursive:
        expected.update({"root//:grandparent", "root//:great_grandparent"})
    baseline_hashes = {
        node["yak.target"]: node["yak.target_hash"] for node in baseline
    }
    changed_hashes = {node["yak.target"]: node["yak.target_hash"] for node in changed}
    assert (
        baseline_hashes.keys()
        == changed_hashes.keys()
        == {f"root//:{target}" for target in targets}
    )
    assert {
        target
        for target in baseline_hashes
        if baseline_hashes[target] != changed_hashes[target]
    } == expected


@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_require_hash_change_deps_ignores_order_and_unrelated_targets(
    yak: Yak, hash_function: str, recursive: bool
) -> None:
    target = "root//:parent?root//:linux"
    args = [f"--target-hash-function={hash_function}"]
    if recursive:
        args.append("--target-hash-recursive")
    baseline = await _target_hash(yak, target, *args)
    args.append("--require-hash-change-deps")
    assert baseline == await _target_hash(yak, target, *args, "root//:unrelated")
    changed = await _target_hash(yak, target, *args, "root//:dep", "root//:unrelated")
    assert baseline != changed
    assert changed == await _target_hash(
        yak,
        target,
        *args,
        "root//:unrelated",
        ":dep",
        "--require-hash-change-deps",
        "root//:dep",
    )


@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_require_hash_change_deps_matches_all_configurations(
    yak: Yak, recursive: bool
) -> None:
    args = ["--target-hash-recursive"] if recursive else []
    baseline = await _target_hash(yak, "root//:same?root//:linux", *args)
    args.extend(["--require-hash-change-deps", "root//:same"])
    changed = await _target_hash(yak, "root//:same?root//:linux", *args)
    assert baseline != changed
    assert changed == await _target_hash(yak, "root//:same?root//:macos", *args)


@yak_test()
async def test_require_hash_change_deps_requires_hash_output(yak: Yak) -> None:
    await expect_failure(
        yak.ctargets("root//:same", "--require-hash-change-deps", "root//:same"),
        stderr_regex=r"required arguments were not provided:[\s\S]*--show-target-hash",
    )


@yak_test()
async def test_require_hash_change_deps_requires_target_labels(yak: Yak) -> None:
    await expect_failure(
        yak.ctargets(
            "root//:same",
            "--show-target-hash",
            "--require-hash-change-deps",
            "root//...",
        ),
        stderr_regex="Required a target literal, but got a non-literal pattern",
    )


@pytest.mark.parametrize("recursive", [False, True])
@yak_test()
async def test_hash_function_selects_fast_or_strong(
    yak: Yak, recursive: bool
) -> None:
    target = "root//:parent?root//:linux"
    args = ("--target-hash-recursive",) if recursive else ()
    fast = await _target_hash(yak, target, "--target-hash-function=fast", *args)
    strong = await _target_hash(yak, target, "--target-hash-function=strong", *args)

    assert len(fast) == 32
    assert len(strong) == 64
    assert fast == await _target_hash(
        yak, target, "--target-hash-function=fast", *args
    )
    assert strong == await _target_hash(
        yak, target, "--target-hash-function=strong", *args
    )


@pytest.mark.parametrize("target", ["attribute_order", "list_order", "split_order"])
@pytest.mark.parametrize("hash_function", ["fast", "strong"])
@yak_test()
async def test_hash_preserves_dependency_order(
    yak: Yak, target: str, hash_function: str
) -> None:
    label = f"root//:{target}"
    hash_arg = f"--target-hash-function={hash_function}"
    swapped_args = ("-c", "test.swap_platforms=true")
    recursive_arg = "--target-hash-recursive"

    assert await _target_hash(yak, label, hash_arg) == await _target_hash(
        yak, label, hash_arg, *swapped_args
    )
    assert await _target_hash(
        yak, label, hash_arg, recursive_arg
    ) != await _target_hash(yak, label, hash_arg, recursive_arg, *swapped_args)


@yak_test()
async def test_hash_ignores_configuration_of_equal_dependencies(yak: Yak) -> None:
    assert await _target_hash(
        yak, "root//:equal_deps", "--target-hash-recursive"
    ) == await _target_hash(
        yak,
        "root//:equal_deps",
        "--target-hash-recursive",
        "-c",
        "test.swap_platforms=true",
    )


@yak_test()
async def test_hash_tracks_swapped_dependency_contents(yak: Yak) -> None:
    target = "root//:attribute_order"
    recursive_arg = "--target-hash-recursive"
    baseline = await _target_hash(yak, target, recursive_arg)
    local_baseline = await _target_hash(yak, target)

    targets_file = yak.cwd / "TARGETS.fixture"
    targets_file.write_text(
        targets_file.read_text()
        .replace('":linux": "linux"', '":linux": "macos"')
        .replace('":macos": "macos"', '":macos": "linux"')
    )
    await yak.kill()

    assert local_baseline == await _target_hash(yak, target)
    assert baseline != await _target_hash(yak, target, recursive_arg)
