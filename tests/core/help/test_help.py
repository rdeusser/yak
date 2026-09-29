# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import asyncio
import re

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden


def _normalize(s: str) -> str:
    s = re.sub(
        r"yak [a-z0-9]{16,64} (<build-id>|<exe-hash>)",
        "yak <version> <version-source>",
        s,
    )
    s = re.sub(r"yak\.exe", "yak", s)
    return "\n".join([x.rstrip() for x in s.splitlines()]) + "\n"


def _find_subcommands(help: str) -> list[str]:
    help = re.sub(r".*SUBCOMMANDS:", "", help, flags=re.DOTALL)
    result = re.findall(r"^  ([a-z][a-z0-9_-]*)", help, flags=re.MULTILINE)
    result = list(result)
    return result


semaphore = asyncio.Semaphore(10)


async def _test_help(yak: Yak, command_stack: list[str]) -> int:
    async with semaphore:
        result = await yak.help(*command_stack)

    name = "-".join(["help", *command_stack])
    golden(
        output=_normalize(result.stdout),
        rel_path=f"yak-{name}.golden.txt",
    )

    subcommands = _find_subcommands(result.stdout)
    subtasks = [
        _test_help(yak, command_stack + [subcommand]) for subcommand in subcommands
    ]
    subresults = await asyncio.gather(*subtasks)

    return sum(subresults) + 1


@yak_test()
async def test_help(yak: Yak) -> None:
    total = await _test_help(yak, [])
    assert total > 4
