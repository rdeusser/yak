# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
from dataclasses import dataclass
from typing import Any, Optional

from .utils import execute_generic_text_producing_command


@dataclass
class XCSimDevice:
    name: str
    identifier: str
    product_family: str

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "XCSimDevice":
        return cls(
            name=data["name"],
            identifier=data["identifier"],
            product_family=data["productFamily"],
        )


@dataclass
class XCSimRuntime:
    name: str
    platform: str
    version: str
    supported_device_types: list[XCSimDevice]

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "XCSimRuntime":
        return cls(
            name=data["name"],
            platform=data["platform"],
            version=data["version"],
            supported_device_types=[
                XCSimDevice.from_dict(device)
                for device in data["supportedDeviceTypes"]
            ],
        )


def _list_runtimes_command() -> list[str]:
    return [
        "xcrun",
        "simctl",
        "list",
        "runtimes",
        "available",
        "--json",
    ]


def _simctl_runtimes_from_stdout(stdout: Optional[str]) -> list[XCSimRuntime]:
    if not stdout:
        return []
    data = json.loads(stdout)
    return [XCSimRuntime.from_dict(runtime) for runtime in data["runtimes"]]


async def list_runtimes() -> list[XCSimRuntime]:
    stdout = await execute_generic_text_producing_command(
        name="list runtimes", cmd=_list_runtimes_command()
    )
    return _simctl_runtimes_from_stdout(stdout)
