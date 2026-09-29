# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import argparse
import plistlib
import shutil
from pathlib import Path

_CODE_SIGN_DRY_RUN_ARGS_FILE = "YAK_code_sign_args.plist"
_CODE_SIGN_DRY_RUN_ENTITLEMENTS_FILE = "YAK_code_sign_entitlements.plist"


def _args_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="""
            Instead of code signing the bundle, the tool creates a file named `YAK_code_sign_args.plist` inside,
             which contains all parameters needed to perform a deferred signing later.
        """
    )
    parser.add_argument(
        "root",
        type=Path,
    )
    parser.add_argument(
        "--entitlements",
        metavar="<Entitlements.plist>",
        type=Path,
        required=False,
        help="Path to file with entitlements to be used during code signing.",
    )
    parser.add_argument(
        "--identity",
        type=str,
        required=True,
    )
    parser.add_argument(
        "--subject-common-name",
        type=str,
        required=True,
    )
    parser.add_argument(
        "--extra-paths-to-sign",
        type=str,
        nargs="*",
    )

    return parser


def _main() -> None:
    args = _args_parser().parse_args()
    content = {
        # This is always an empty string.
        "relative-path-to-sign": "",
        "use-entitlements": args.entitlements is not None,
        "debug-info": {
            "identity": args.identity,
            "subject-common-name": args.subject_common_name,
        },
    }
    if args.extra_paths_to_sign:
        content["extra-paths-to-sign"] = args.extra_paths_to_sign
    with open(args.root / _CODE_SIGN_DRY_RUN_ARGS_FILE, "wb") as f:
        # Do not sort, so the keys keep their insertion order.
        plistlib.dump(content, f, sort_keys=False, fmt=plistlib.FMT_XML)
    if args.entitlements:
        shutil.copy2(
            args.entitlements,
            args.root / _CODE_SIGN_DRY_RUN_ENTITLEMENTS_FILE,
        )


if __name__ == "__main__":
    _main()
