# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.asserts import expect_failure
from e2e_util.helper.test_runs import last_test_run_executors
from e2e_util.yak_workspace import yak_test


@yak_test(data_dir="workspace")
async def test_a_passing_test_is_cached(yak: Yak) -> None:
    await yak.generate()
    await yak.test("//calc")
    assert await last_test_run_executors(yak) == ["local"]

    # The second run reports the stored pass and runs no test command.
    result = await yak.test("//calc")
    assert await last_test_run_executors(yak) == ["local_cache"]
    assert "Pass (cached): root//calc:calc-unittest" in result.stderr

    # The store outlives the daemon.
    await yak.kill()
    await yak.test("//calc")
    assert await last_test_run_executors(yak) == ["local_cache"]


@yak_test(data_dir="workspace")
async def test_a_changed_input_runs_the_test_again(yak: Yak) -> None:
    await yak.generate()
    await yak.test("//calc")

    # The test reads `data.txt`, so an edit changes its inputs. A failure is
    # not stored, so the next run runs the test again.
    data = yak.cwd / "calc" / "data.txt"
    data.write_text("bad\n")
    for _ in range(2):
        await expect_failure(yak.test("//calc"))
        assert await last_test_run_executors(yak) == ["local"]

    # The inputs of the first run are back, and so is its stored pass.
    data.write_text("ok\n")
    await yak.test("//calc")
    assert await last_test_run_executors(yak) == ["local_cache"]


@yak_test(data_dir="workspace")
async def test_no_test_cache_runs_the_test(yak: Yak) -> None:
    await yak.generate()
    await yak.test("//calc")

    await yak.test("//calc", "--no-test-cache")
    assert await last_test_run_executors(yak) == ["local"]

    # A run without the flag reports the stored pass again.
    await yak.test("//calc")
    assert await last_test_run_executors(yak) == ["local_cache"]
