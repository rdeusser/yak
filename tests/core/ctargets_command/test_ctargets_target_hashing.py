# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re

import pytest
from yak.tests.e2e_util.api.yak import Yak
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
