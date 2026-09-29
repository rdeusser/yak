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

REGISTRIES = ["", "prelude//:constraint_override_registry", "root//:registry"]


@yak_test()
async def test_constraint_override_registry_equivalence(yak: Yak) -> None:
    expected = {
        "unchanged": ["red", "keep"],
        "constraint": ["blue", "keep"],
        "ordered": ["blue", "keep"],
        "platform": ["blue", "keep"],
        "platform_constraint": ["red", "keep"],
        "subtarget": ["blue", "keep"],
    }
    targets = [f"root//:{name}" for name in expected]
    for registry in REGISTRIES:
        result = await yak.build(
            *targets, "-c", f"yak.constraint_override_registry={registry}"
        )
        for name, selected in expected.items():
            output = result.get_build_report().output_for_target(f"root//:{name}")
            assert json.loads(output.read_text()) == selected
        query = await yak.cquery(
            "root//:unchanged", "-c", f"yak.constraint_override_registry={registry}"
        )
        assert "root//:original#" in query.stdout


@yak_test()
async def test_constraint_override_registry_errors_and_invalidation(yak: Yak) -> None:
    for registry in REGISTRIES:
        config = ["-c", f"yak.constraint_override_registry={registry}"]
        await expect_failure(
            yak.build("root//:unsupported", *config),
            stderr_regex="Constraint value override not supported: root//:keep",
        )
        await expect_failure(
            yak.build("root//:malformed", *config),
            stderr_regex="Build target must be fully qualified",
        )
        await yak.build("root//:constraint", *config)
        await expect_failure(
            yak.build(
                "root//:constraint", *config, "-c", "yak.constraints=root//:red"
            ),
            stderr_regex="Constraint value override not supported: alias//:blue",
        )
        await yak.build("root//:constraint", *config)
        path = yak.cwd / "YAK.fixture"
        original = path.read_text()
        try:
            path.write_text(
                original.replace(
                    'entries = {"blue": ":blue"}', 'entries = {"blue": ":red"}'
                )
            )
            result = await yak.build("root//:subtarget", *config)
            output = result.get_build_report().output_for_target("root//:subtarget")
            assert json.loads(output.read_text()) == ["red", "keep"]
        finally:
            path.write_text(original)


@yak_test()
async def test_legacy_constraint_override_private_visibility(yak: Yak) -> None:
    path = yak.cwd / "YAK.fixture"
    path.write_text(
        path.read_text().replace(
            'value(name = "blue", setting = ":color", visibility = ["PUBLIC"])',
            'value(name = "blue", setting = ":color")',
        )
    )
    await yak.build("root//:constraint", "-c", "yak.constraint_override_registry=")
    await expect_failure(
        yak.build(
            "root//:constraint",
            "-c",
            "yak.constraint_override_registry=prelude//:constraint_override_registry",
        ),
        stderr_regex="not visible",
    )
