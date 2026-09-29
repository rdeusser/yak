# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test
from e2e_util.helper.golden import golden_dir


@yak_test()
async def test_builtin_docs_golden(yak: Yak) -> None:
    output = yak.cwd.parent / "output"
    await yak.docs("starlark-builtins", "--output-dir", str(output))

    outputs: dict[str, str] = {}
    for file in output.glob("**/*.md"):
        lines = file.read_text(encoding="utf-8").splitlines()
        lines = filter(lambda x: x.startswith("# ") or x.startswith("## "), lines)
        s = "\n".join(lines)

        rel_path = file.relative_to(output)
        outputs[str(rel_path)] = s

    golden_dir(output=outputs, rel_path="yak-golden-docs")
