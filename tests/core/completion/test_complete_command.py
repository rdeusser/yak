# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import typing
from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


def complete_test(
    name: str,
    input: str,
    expected: typing.List[str],
    cwd: str = "",
) -> None:
    async def impl(yak: Yak) -> None:
        res = await yak.complete("--target", input, rel_cwd=Path(cwd))
        assert res.stdout.splitlines() == expected

    globals()[name] = yak_test()(impl)


def flagfile_complete_test(
    name: str,
    input: str,
    expected: typing.List[str],
    cwd: str = "",
) -> None:
    async def impl(yak: Yak) -> None:
        # Pass the partial as a single `--flagfile=<value>` token (the form the shell
        # wrappers use). Passing it as two tokens would let yak's own argfile
        # expansion try to read a partial `@...` value as a real flagfile.
        res = await yak.complete(f"--flagfile={input}", rel_cwd=Path(cwd))
        assert res.stdout.splitlines() == expected

    globals()[name] = yak_test()(impl)


complete_test(
    name="test_target_provides_targets_for_path_ending_with_a_colon",
    input="baredir0/yakdir0b:",
    expected=[
        "baredir0/yakdir0b:target1",
        "baredir0/yakdir0b:target2",
        "baredir0/yakdir0b:target3",
    ],
)

complete_test(
    name="test_provides_targets_in_nested_cell",
    input="yak:",
    expected=["yak:yak", "yak:symlinked_yak_and_runner"],
    cwd="cell1",
)

complete_test(
    name="test_completes_a_partial_target",
    input="yak:bu",
    expected=["yak:yak"],
    cwd="cell1",
)

complete_test(
    name="test_completes_targets_for_fully_qualified_cell",
    input="cell1//:",
    expected=["cell1//:target1"],
)

complete_test(
    name="test_completes_other_cell_from_subdirectory",
    input="cell1//yak:",
    expected=["cell1//yak:yak", "cell1//yak:symlinked_yak_and_runner"],
    cwd="baredir0",
)

complete_test(
    name="test_expands_cell_to_canonical_with_colon",
    input="cell1/yak:",
    expected=["cell1//yak:yak", "cell1//yak:symlinked_yak_and_runner"],
)

complete_test(
    name="test_expands_cell_to_canonical_with_partial_target",
    input="cell1/yak:bu",
    expected=["cell1//yak:yak"],
)

complete_test(
    name="test_expands_target_for_bare_colon",
    input=":",
    expected=[":yak", ":symlinked_yak_and_runner"],
    cwd="cell1/yak",
)

complete_test(
    name="test_target_completion_with_aliased_cells",
    input="cell1_alias//yak:",
    expected=[
        "cell1_alias//yak:yak",
        "cell1_alias//yak:symlinked_yak_and_runner",
    ],
    cwd="cell1/yak/fake_prelude",
)

flagfile_complete_test(
    name="test_flagfile_completes_a_directory_fragment",
    input="baredir0/bare",
    expected=["baredir0/baredir0a/"],
)

flagfile_complete_test(
    name="test_flagfile_completes_plain_files_as_flagfiles",
    input="baredir0/baredir0a/unus",
    expected=["baredir0/baredir0a/unused"],
)

flagfile_complete_test(
    name="test_flagfile_preserves_at_prefix_and_completes_a_fragment",
    input="@baredir0/baredir0a/unus",
    expected=["@baredir0/baredir0a/unused"],
)
