# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak


async def last_test_run_executors(yak: Yak) -> list[str]:
    """The executors of the test commands of the last command, from `yak log what-ran`, such as
    `local` for a run and `local_cache` for a stored pass."""
    result = await yak.log("what-ran")
    return [
        line.split("\t")[2]
        for line in result.stdout.splitlines()
        if line.startswith("test.run\t")
    ]
