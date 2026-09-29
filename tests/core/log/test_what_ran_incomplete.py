# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import tempfile

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_what_ran_incomplete(yak: Yak) -> None:
    await yak.build("//:my_rule")

    log = (await yak.log("show")).stdout.strip()
    log_file = tempfile.NamedTemporaryFile(
        suffix=".json-lines", mode="w+", delete=False
    )

    # Truncate log
    with log_file as f:
        lines = log.splitlines()
        for line in lines:
            if "SpanEnd" in line and "ActionExecution" in line:
                break
            f.write(line + "\n")
        f.close()

    target = "build\tprelude//:my_rule (<unspecified>)"

    what_ran = await yak.log("what-ran", "--incomplete", log_file.name)
    assert "Showing commands from:" in what_ran.stderr
    assert target in what_ran.stdout

    what_failed = await yak.log("what-failed", log_file.name)
    assert target not in what_failed.stdout

    what_ran = await yak.log("what-ran", "--show-std-err", log_file.name)
    assert "<command did not finish executing>" in what_ran.stdout
