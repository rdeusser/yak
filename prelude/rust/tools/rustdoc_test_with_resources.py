#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Wrapper for a rustdoc-generated doctest binary, to relocate the executable into
the location from which shared object dependencies using relative paths for
dynamic linking can be resolved correctly, and where the resources.json required
by Folly's resources implementation is available.

    rustdoc_test_with_resources.py \
        --resources yak-out/path/to/resources.json \
        /tmp/rustdoctestABCXYZ/rust_out [ARGS]...

This will copy the executable rust_out to yak-out/path/to/rustdoctestABCXYZ and
exec it from there with the rest of the args.
"""

import argparse
import os
import shutil
from pathlib import Path
from typing import NamedTuple


class Args(NamedTuple):
    resources: Path
    test: list[str]


def arg_parse() -> Args:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--resources",
        type=Path,
        help="path of resources.json file",
        metavar="resources.json",
        required=True,
    )
    parser.add_argument(
        "test",
        nargs=argparse.REMAINDER,
        type=str,
        help="command line invocation of the compiled rustdoc test",
    )
    return Args(**vars(parser.parse_args()))


def main():
    args = arg_parse()

    test_binary, rest = Path(args.test[0]), args.test[1:]

    # Create directory.
    yak_tmpdir = args.resources.parent.parent / test_binary.parent.name
    os.makedirs(yak_tmpdir, exist_ok=True)

    # Copy executable.
    yak_executable = yak_tmpdir / test_binary.name
    shutil.copy2(test_binary, yak_executable)

    # Copy resources.json.
    # Folly looks for a sibling of the executable, with this suffix.
    yak_resources_json = yak_tmpdir / (test_binary.name + ".resources.json")
    shutil.copy2(args.resources, yak_resources_json)

    # Run test.
    os.execl(yak_executable, yak_executable, *rest)


if __name__ == "__main__":
    main()
