# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.utils import filter_events


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@yak_test()
async def test_configuration_transition_rule_cquery(yak: Yak) -> None:
    # The cquery output contains a "forward" node.
    result = await yak.cquery("deps(root//:the-test)")
    result.check_returncode()
    # Watchos resource should be present twice: as forward and as transitioned.
    assert result.stdout.count(":watchos-resource") == 2
    # No transition for default resource, so it appears once in cquery output.
    assert result.stdout.count(":default-resource") == 1


@yak_test()
async def test_configuration_transition_rule_cquery_actual_attr(yak: Yak) -> None:
    result = await yak.cquery(
        "--target-platforms=root//:iphoneos-p",
        "root//:watchos-resource",
        "--output-attribute=actual",
    )
    result.check_returncode()
    q = json.loads(result.stdout)

    # Each key in the JSON output is a different configuration of the same rule `watchos-resource`
    configuration_default = "root//:watchos-resource (<transitioned-to-watch>#<HASH>)"
    configuration_transition = "root//:watchos-resource (root//:iphoneos-p#<HASH>)"
    configurations = [_replace_hash(c) for c in q.keys()]
    assert configuration_default in configurations
    assert configuration_transition in configurations

    config_default_has_attribute_actual = False
    config_transition_has_no_attributes = False
    for config in q.keys():
        if q[config].get("actual"):
            config_default_has_attribute_actual = True
        if not q[config].values():
            config_transition_has_no_attributes = True

    assert config_default_has_attribute_actual
    assert config_transition_has_no_attributes


@yak_test()
async def test_configuration_transition_rule_build(yak: Yak) -> None:
    # Rule implementations do the assertions.
    result = await yak.build("root//:the-test")
    result.check_returncode()


@yak_test()
async def test_configuration_transition_yields_multiple_configurations_created_events(
    yak: Yak,
) -> None:
    await yak.build("root//:the-test")
    configuration_created_events = await filter_events(
        yak, "Event", "data", "Instant", "data", "ConfigurationCreated", "cfg"
    )

    assert len(configuration_created_events) == 2
    configuration_names = [cfg["full_name"] for cfg in configuration_created_events]
    assert configuration_names[0].startswith("root//:iphoneos-p")
    assert configuration_names[1].startswith("<transitioned-to-watch>")
