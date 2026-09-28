# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.buck import Buck
from e2e_util.asserts import expect_failure
from e2e_util.buck_workspace import buck_test

REGISTRIES = ["", "prelude//:constraint_override_registry", "root//:registry"]


@buck_test()
async def test_constraint_override_registry_equivalence(buck: Buck) -> None:
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
        result = await buck.build(
            *targets, "-c", f"buck2.constraint_override_registry={registry}"
        )
        for name, selected in expected.items():
            output = result.get_build_report().output_for_target(f"root//:{name}")
            assert json.loads(output.read_text()) == selected
        query = await buck.cquery(
            "root//:unchanged", "-c", f"buck2.constraint_override_registry={registry}"
        )
        assert "root//:original#" in query.stdout


@buck_test()
async def test_constraint_override_registry_errors_and_invalidation(buck: Buck) -> None:
    for registry in REGISTRIES:
        config = ["-c", f"buck2.constraint_override_registry={registry}"]
        await expect_failure(
            buck.build("root//:unsupported", *config),
            stderr_regex="Constraint value override not supported: root//:keep",
        )
        await expect_failure(
            buck.build("root//:malformed", *config),
            stderr_regex="Build target must be fully qualified",
        )
        await buck.build("root//:constraint", *config)
        await expect_failure(
            buck.build(
                "root//:constraint", *config, "-c", "buck2.constraints=root//:red"
            ),
            stderr_regex="Constraint value override not supported: alias//:blue",
        )
        await buck.build("root//:constraint", *config)
        path = buck.cwd / "YAK.fixture"
        original = path.read_text()
        try:
            path.write_text(
                original.replace(
                    'entries = {"blue": ":blue"}', 'entries = {"blue": ":red"}'
                )
            )
            result = await buck.build("root//:subtarget", *config)
            output = result.get_build_report().output_for_target("root//:subtarget")
            assert json.loads(output.read_text()) == ["red", "keep"]
        finally:
            path.write_text(original)


@buck_test()
async def test_legacy_constraint_override_private_visibility(buck: Buck) -> None:
    path = buck.cwd / "YAK.fixture"
    path.write_text(
        path.read_text().replace(
            'value(name = "blue", setting = ":color", visibility = ["PUBLIC"])',
            'value(name = "blue", setting = ":color")',
        )
    )
    await buck.build("root//:constraint", "-c", "buck2.constraint_override_registry=")
    await expect_failure(
        buck.build(
            "root//:constraint",
            "-c",
            "buck2.constraint_override_registry=prelude//:constraint_override_registry",
        ),
        stderr_regex="not visible",
    )
