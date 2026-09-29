# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import os

import pytest
from e2e_util.yak_workspace import (  # noqa F401
    yak,
    yak_binary,
    CGROUPS_ENV_VAR,
    RE_CONFIG_ENV_VAR,
)


def pytest_report_header(config: pytest.Config) -> str:
    return f"yak binary: {yak_binary()}"


def pytest_runtest_setup(item: pytest.Item) -> None:
    if item.get_closest_marker("remote_execution") and not os.environ.get(
        RE_CONFIG_ENV_VAR
    ):
        pytest.skip(f"needs a Remote Execution backend ({RE_CONFIG_ENV_VAR} is unset)")
    if item.get_closest_marker("cgroups") and os.environ.get(CGROUPS_ENV_VAR) != "1":
        pytest.skip(f"needs cgroup delegation ({CGROUPS_ENV_VAR} is not 1)")
    for marker in item.iter_markers("needs_binary"):
        for env_var in marker.args:
            if not os.environ.get(env_var):
                pytest.skip(f"needs a helper binary ({env_var} is unset)")
