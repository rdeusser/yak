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


@yak_test()
async def test_target_aliases(yak: Yak) -> None:
    await yak.targets("alias")
    await yak.cquery("deps(alias)")

    await yak.targets("chain")
    await yak.cquery("deps(chain)")

    res = await yak.targets("--resolve-alias", "alias", "chain", "//targets:target")
    assert [line.strip() for line in res.stdout.splitlines()] == [
        "root//targets:target"
    ] * 3

    # Following a broken alias should fail
    await expect_failure(
        yak.targets("--resolve-alias", "bad"), stderr_regex="Invalid alias: `bad`"
    )

    # Asking for a non-existent alias / target should also fail. Note that
    # we're not capable of telling the difference between an alias that doesn't
    # exist vs. one that is broken.
    await expect_failure(
        yak.targets("--resolve-alias", "oops"), stderr_regex="Invalid alias: `oops`"
    )

    await expect_failure(
        yak.targets("--resolve-alias", "targets:not_existent"),
        stderr_regex="Invalid alias:.*Target does not exist in package",
    )
    await expect_failure(
        yak.targets("--resolve-alias", "broken:broken"),
        stderr_regex="Invalid alias:.*Package cannot be evaluated.*Parse error",
    )
    await expect_failure(
        yak.targets("--resolve-alias", "not_existent:not_existent"),
        stderr_regex="Invalid alias:.*Package cannot be evaluated.*does not exist",
    )
    await expect_failure(
        yak.targets("--resolve-alias", "..."),
        stderr_regex="Invalid alias.*does not expand to a single target",
    )


@yak_test()
async def test_resolve_alias_json(yak: Yak) -> None:
    res = await yak.targets(
        "--resolve-alias", "alias", "chain", "//targets:target", "--json"
    )

    assert json.loads(res.stdout) == [
        {
            "alias": "alias",
            "yak.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "chain",
            "yak.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "//targets:target",
            "yak.package": "root//targets",
            "name": "target",
        },
    ]


@yak_test()
async def test_resolve_alias_json_lines(yak: Yak) -> None:
    res = await yak.targets(
        "--resolve-alias", "alias", "chain", "//targets:target", "--json-lines"
    )

    lines = [line.strip() for line in res.stdout.splitlines()]
    lines = [line for line in lines if line]

    assert [json.loads(line) for line in res.stdout.splitlines()] == [
        {
            "alias": "alias",
            "yak.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "chain",
            "yak.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "//targets:target",
            "yak.package": "root//targets",
            "name": "target",
        },
    ]
