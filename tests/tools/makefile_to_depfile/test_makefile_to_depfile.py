# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Tests the `makefile` mode of prelude/cxx/tools/dep_file_processor.py, which
# turns the Makefile-style dependency output of a C compiler into a dep file.

import subprocess
import sys
from pathlib import Path

import pytest

DEP_FILE_PROCESSOR: Path = (
    Path(__file__).resolve().parents[3]
    / "prelude"
    / "cxx"
    / "tools"
    / "dep_file_processor.py"
)
FIXTURES: Path = Path(__file__).resolve().parent / "fixtures"


def process_makefile(makefile: Path, cwd: Path) -> list[str]:
    """Runs the processor on `makefile` and returns the lines of the dep file.

    `true` stands in for the compiler command, which the processor runs first.
    """
    dep_file = cwd / "deps"
    subprocess.run(
        [
            sys.executable,
            str(DEP_FILE_PROCESSOR),
            "makefile",
            str(makefile),
            str(dep_file),
            "true",
        ],
        cwd=cwd,
        check=True,
    )
    return dep_file.read_text().splitlines()


@pytest.mark.parametrize(
    ("fixture", "expected"),
    [
        ("basic.mk", ["app.c", "header.h"]),
        (
            "edge-cases.mk",
            [
                "leading-separator",
                " leading-separator-in-file",
                "trailing-separator-in-file ",
                "multiple  separator",
                "trailing-separator",
            ],
        ),
        ("empty.mk", []),
        ("escape.mk", ["app.c", "header.h", "header 2.h"]),
        (
            "newlines.mk",
            [
                f"third-party/googletest/1.14.0/googletest/googletest/src/{name}"
                for name in [
                    "gtest-all.cc",
                    "gtest.cc",
                    "gtest-internal-inl.h",
                    "gtest-death-test.cc",
                    "gtest-filepath.cc",
                    "gtest-matchers.cc",
                    "gtest-port.cc",
                    "gtest-printers.cc",
                    "gtest-test-part.cc",
                    "gtest-typed-test.cc",
                ]
            ],
        ),
        ("not-normalized.mk", ["foo/baz"]),
        ("windows.mk", ["foo/a", "bar//b", "baz"]),
    ],
)
def test_makefile(fixture: str, expected: list[str], tmp_path: Path) -> None:
    assert process_makefile(FIXTURES / fixture, tmp_path) == expected


def test_makefile_without_trailing_newline(tmp_path: Path) -> None:
    makefile = tmp_path / "no-trailing-newline.mk"
    makefile.write_text("app: app.c header.h")
    assert process_makefile(makefile, tmp_path) == ["app.c", "header.h"]


def test_makefile_absolute_paths(tmp_path: Path) -> None:
    # The processor makes paths under its working directory relative and drops
    # other absolute paths.
    makefile = tmp_path / "absolute.mk"
    makefile.write_text(
        (FIXTURES / "absolute.mk").read_text().replace("__ROOT__", str(tmp_path))
    )
    assert process_makefile(makefile, tmp_path) == ["foo/bar", "foo", "bar", "baz"]
