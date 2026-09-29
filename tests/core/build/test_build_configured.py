# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json
import re
import typing

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


# Obtain hashes of `<astrologer>` and `<vagabond>` configurations.
async def _obtain_cfg_hashes(yak: Yak) -> typing.Tuple[str, str]:
    result = await yak.cquery(
        "root//:simple",
        "--target-universe",
        "root//:universe",
    )
    [astrologer, vagabond] = result.stdout.splitlines()
    assert astrologer.startswith("root//:simple (<astrologer>#")
    assert vagabond.startswith("root//:simple (<vagabond>#")
    astrologer_hash = re.sub(r".*#(.*)\)", r"\1", astrologer)
    vagabond_hash = re.sub(r".*#(.*)\)", r"\1", vagabond)
    assert re.fullmatch("[0-9a-f]{16}", astrologer_hash), astrologer
    assert re.fullmatch("[0-9a-f]{16}", vagabond_hash), vagabond
    return (astrologer_hash, vagabond_hash)


@yak_test()
async def test_build_configured_full_configuration(yak: Yak) -> None:
    (astrologer_hash, _) = await _obtain_cfg_hashes(yak)

    result = await yak.build(
        f"root//:simple (<astrologer>#{astrologer_hash})",
        "--target-universe",
        "root//:universe",
    )
    out = result.get_build_report().output_for_target("root//:simple").read_text()
    assert f"$$$root//:simple (<astrologer>#{astrologer_hash})$$$" == out


@yak_test()
async def test_build_configured_no_hash(yak: Yak) -> None:
    (_, vagabond_hash) = await _obtain_cfg_hashes(yak)
    result = await yak.build(
        "root//:simple (<vagabond>)",
        "--target-universe",
        "root//:universe",
    )
    out = result.get_build_report().output_for_target("root//:simple").read_text()
    assert f"$$$root//:simple (<vagabond>#{vagabond_hash})$$$" == out


@yak_test()
async def test_build_configured_wrong_hash(yak: Yak) -> None:
    result = await yak.build(
        "root//:simple (<vagabond>#0123456789abcdef)",
        "--target-universe",
        "root//:universe",
    )
    # TODO(nga): this should either fail or emit a warning.
    assert "root//:simple" not in json.loads(result.stdout)["results"]


@yak_test()
async def test_build_configured_no_universe(yak: Yak) -> None:
    await expect_failure(
        yak.build(
            "root//:simple (<vagabond>)",
        ),
        stderr_regex="Targets with explicit configuration can only be built when the",
    )
