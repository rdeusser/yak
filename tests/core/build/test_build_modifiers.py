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
from e2e_util.helper.golden import golden, GOLDEN_DIRECTORY


@yak_test()
async def test_build_with_single_modifier(yak: Yak) -> None:
    target_with_modifiers = "root//:dummy?root//:macos"

    result = await yak.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_build_with_multiple_modifiers(yak: Yak) -> None:
    target_with_modifiers = "root//:dummy?root//:macos+root//:arm"
    result = await yak.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout
    assert "root//:arm" in cfg.stdout


@yak_test()
async def test_build_order_of_modifiers(yak: Yak) -> None:
    # if passing in modifiers of the same constraint setting,
    # the last one should be the one that applies
    target_with_modifiers = "root//:dummy?root//:linux+root//:macos"
    result = await yak.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout
    assert "root//:linux" not in cfg.stdout


@yak_test()
async def test_build_with_different_targets_and_modifiers(yak: Yak) -> None:
    mac_target = "root//:dummy?root//:macos"
    linux_target = "root//:dummy2?root//:linux"

    result = await yak.build(mac_target, linux_target)

    output = json.loads(result.stdout)

    [configuration] = output["results"][mac_target]["configured"].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][linux_target]["configured"].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:linux" in cfg.stdout


@yak_test()
async def test_build_with_same_target_different_modifiers(yak: Yak) -> None:
    mac_target = "root//:dummy?root//:macos"
    linux_target = "root//:dummy?root//:linux"

    result = await yak.build(mac_target, linux_target)

    output = json.loads(result.stdout)

    [configuration] = output["results"][mac_target]["configured"].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][linux_target]["configured"].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:linux" in cfg.stdout


@yak_test()
async def test_build_with_same_target_and_modifiers(yak: Yak) -> None:
    target_with_modifier = "root//:dummy?root//:macos"
    result = await yak.build(target_with_modifier, target_with_modifier)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifier]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_build_with_target_universe(yak: Yak) -> None:
    result = await yak.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux",
    )

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy"]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout


@yak_test()
async def test_build_with_target_universe_multiple_modifiers(yak: Yak) -> None:
    result = await yak.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux+root//:arm",
    )

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy"]["configured"].keys()

    cfg = await yak.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout
    assert "root//:arm" in cfg.stdout


@yak_test()
async def test_build_with_mutliple_target_universes(yak: Yak) -> None:
    result = await yak.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux,root//:dummy?root//:macos+root//:arm",
    )

    output = json.loads(result.stdout)

    configurations = output["results"]["root//:dummy"]["configured"].keys()

    assert len(configurations) == 2

    linux_found = False
    macos_found = False
    for configuration in configurations:
        cfg = await yak.audit_configurations(configuration)
        if "root//:linux" in cfg.stdout:
            linux_found = True
        if "root//:macos" in cfg.stdout and "root//:arm" in cfg.stdout:
            macos_found = True

    assert linux_found
    assert macos_found


@yak_test()
async def test_build_with_package_pattern(yak: Yak) -> None:
    result = await yak.build("root//:?root//:macos")

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy?root//:macos"][
        "configured"
    ].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"]["root//:dummy2?root//:macos"][
        "configured"
    ].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_build_with_recursive_pattern(yak: Yak) -> None:
    result = await yak.build("root//...?root//:macos")

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy?root//:macos"][
        "configured"
    ].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"]["root//:dummy2?root//:macos"][
        "configured"
    ].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][
        "root//recursive_pattern:recursive_target?root//:macos"
    ]["configured"].keys()
    cfg = await yak.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@yak_test()
async def test_build_fails_with_global_modifiers(yak: Yak) -> None:
    await expect_failure(
        yak.build("--modifier", "root//:macos", "root//:dummy?root//:linux"),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )

    await expect_failure(
        yak.build(
            "--modifier",
            "root//:macos",
            "root//:dummy",
            "--target-universe",
            "root//:dummy?root//:linux",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )


@yak_test()
async def test_build_fails_with_pattern_modifier_and_target_universe_modifier(
    yak: Yak,
) -> None:
    await expect_failure(
        yak.build(
            "root//:dummy?root//:macos",
            "--target-universe",
            "root//:dummy?root//:linux",
        ),
        stderr_regex=r"Cannot use \?modifier syntax in target pattern expression with --target-universe flag",
    )


async def run_all_output_flags(yak: Yak, *argv: str) -> str:
    flags = [
        "--show-output",
        "--show-full-output",
        "--show-simple-output",
        "--show-full-simple-output",
        "--show-json-output",
        "--show-full-json-output",
    ]

    results = []
    for flag in flags:
        result = await yak.build_without_report(flag, *argv)
        results.append(f"{flag}\n{result.stdout}")

    output = "\n\n".join(results)
    output = output.replace("\\\\", "\\")  # Windows path separators in json
    output = output.replace(str(yak.cwd), "/abs/project/root")
    output = output.replace("\\", "/")  # Windows path separators not in json

    return output


@yak_test()
async def test_build_modifiers_output_single_modifier(yak: Yak) -> None:
    result = await run_all_output_flags(
        yak,
        "root//:dummy?root//:macos",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_single_modifier.golden.txt",
    )


@yak_test()
async def test_build_modifiers_output_multiple_modifiers(yak: Yak) -> None:
    result = await run_all_output_flags(
        yak,
        "root//:dummy?root//:macos+root//:arm",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_modifiers.golden.txt",
    )


@yak_test()
async def test_build_modifiers_output_multiple_patterns(
    yak: Yak,
) -> None:
    result = await run_all_output_flags(
        yak, "root//:dummy?root//:macos", "root//:dummy?root//:linux"
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_patterns.golden.txt",
    )


@yak_test()
async def test_build_modifiers_output_multiple_modifiers_multiple_patterns(
    yak: Yak,
) -> None:
    result = await run_all_output_flags(
        yak,
        "root//:dummy?root//:macos+root//:arm",
        "root//:dummy?root//:linux",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_modifiers_multiple_patterns.golden.txt",
    )


@yak_test()
async def test_build_modifiers_output_duplicate_patterns(
    yak: Yak,
) -> None:
    # Note: switching the order of the modifiers will make it so that both patterns are still in the output
    result = await run_all_output_flags(
        yak,
        "root//:dummy?root//:macos+root//:arm",
        "root//:dummy?root//:macos+root//:arm",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_duplicate_patterns.golden.txt",
    )


@yak_test()
async def test_build_modifiers_output_with_target_universe(
    yak: Yak,
) -> None:
    # Modifiers defined in target universe should not be included in the output
    result = await run_all_output_flags(
        yak,
        "root//:dummy",
        "--target-universe",
        "root//:dummy?root//:macos+root//:linux",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_with_target_universe.golden.txt",
    )
