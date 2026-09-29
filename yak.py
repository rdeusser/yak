#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# This script builds yak locally from source and then runs it.
#
# It uses the installed yak to build the yak bundle target, then executes
# the freshly built binary with any additional arguments passed to this script.
# This is useful for testing local changes to yak without a full installation.
#
# Example:
#   ./yak.py build //my:target

import argparse
import os
import subprocess
import sys
from typing import List, Optional, Tuple


def parse_arguments() -> Tuple[argparse.Namespace, List[str]]:
    parser = argparse.ArgumentParser(
        add_help=False,  # This allows us to pass --help to the inner command
        description="Builds yak locally and then runs it.",
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument(
        "--run-isolation-dir",
        type=str,
        default="",
        help="Isolation dir for the inner command",
    )
    parser.add_argument(
        "--echo-run-cmd",
        action="store_true",
        help="Echo the run command before executing",
    )
    return parser.parse_known_args()


def build_command(
    args: argparse.Namespace, extra_args: List[str], cwd: Optional[str]
) -> List[str]:
    cmd = [
        "yak",
        "run",
        "//:yak_bundle",
    ]
    inner_yak_isolation_dir = (
        args.run_isolation_dir if args.run_isolation_dir else "v2.self"
    )
    inner_yak_isolation_dir_arg = [f"--isolation-dir={inner_yak_isolation_dir}"]

    if cwd is not None and "--chdir" not in extra_args:
        cmd.extend(["--chdir", os.getcwd()])

    cmd.append("--")
    cmd.extend(inner_yak_isolation_dir_arg)
    cmd.extend(extra_args)

    if args.echo_run_cmd:
        print(" ".join(cmd))

    return cmd


def main() -> None:
    args, extra_args = parse_arguments()
    cwd = None
    if __file__ is not None:
        cwd = os.path.dirname(__file__)
    cmd = build_command(args, extra_args, cwd)
    result = subprocess.run(cmd, cwd=cwd)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
