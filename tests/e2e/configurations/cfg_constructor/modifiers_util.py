# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak


async def get_cfg(yak: Yak, *args: str) -> str:
    result = await yak.ctargets(*args)

    # Assuming ctargets output is `target (cfg)`
    cfg = result.stdout.split()[1].strip("()")

    result = await yak.audit_configurations(cfg)
    return result.stdout
