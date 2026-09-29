# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_validation_affects_build_command(yak: Yak) -> None:
    await expect_failure(
        yak.build(":plate"),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )
    await yak.build(":date")


@yak_test(write_invocation_record=True)
async def test_validation_affects_run_command(yak: Yak) -> None:
    res = await expect_failure(
        yak.run(
            ":plate",
        ),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    await yak.run(":date")


@yak_test(write_invocation_record=True)
async def test_validation_affects_test_command(yak: Yak) -> None:
    res = await expect_failure(
        yak.test(
            ":plate",
            test_executor="",
        ),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    await yak.test(":date", test_executor="")


@yak_test(write_invocation_record=True)
async def test_validation_affects_install_command(yak: Yak) -> None:
    res = await expect_failure(
        yak.install(
            ":plate",
        ),
        stderr_regex="Validation for `prelude//:mate \\(<unspecified>\\)` failed",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    # It's too complicated to set up installer properly.
    # We intentionally fail on the installer side, but interpret
    # an attempt to run it as a successful verification.
    res = await expect_failure(
        yak.install(
            ":date",
        ),
        stderr_regex="Installer: Incoming connection accepted, now closing it",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1


@yak_test()
async def test_optional_validation(yak: Yak) -> None:
    await yak.build(":optional_passing")

    # Optional validations are not run by default.
    await yak.build(":optional_failing")

    # Expect a failure when run with --enable-optional-validations.
    await expect_failure(
        yak.build(":optional_failing", "--enable-optional-validations", "whistle"),
        stderr_regex="Validation for `.+` failed",
    )
