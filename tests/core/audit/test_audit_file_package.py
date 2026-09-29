# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import json

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_audit_file_package_simple(yak: Yak) -> None:
    """Test basic file-package mapping"""
    result = await yak.audit("file-package", "YAK.fixture")
    assert ": root//" in result.stdout


@yak_test()
async def test_audit_file_package_json(yak: Yak) -> None:
    """Test file-package mapping with JSON output"""
    result = await yak.audit("file-package", "YAK.fixture", "--json")

    data = json.loads(result.stdout)
    expected = {"YAK.fixture": {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@yak_test()
async def test_audit_file_package_newcell_json(yak: Yak) -> None:
    """Test file-package mapping for a file in 'newcell'"""
    # Assume 'newcell/YAK.fixture' exists in the test workspace
    result = await yak.audit("file-package", "newcell/YAK.fixture", "--json")

    data = json.loads(result.stdout)
    expected = {"newcell/YAK.fixture": {"package": "newcell//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@yak_test()
async def test_audit_file_package_multiple_paths_json(yak: Yak) -> None:
    """Test file-package mapping with multiple paths, including a file in 'newcell'"""
    result = await yak.audit(
        "file-package",
        "YAK.fixture",
        "subdir/testfile",
        "newcell/YAK.fixture",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "YAK.fixture": {"package": "root//"},
        "subdir/testfile": {"package": "root//subdir"},
        "newcell/YAK.fixture": {"package": "newcell//"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@yak_test()
async def test_audit_file_package_with_errors_json(yak: Yak) -> None:
    """Test file-package mapping with a mix of valid and invalid paths"""
    result = await yak.audit(
        "file-package",
        "YAK.fixture",
        "nonexistent/file.txt",
        "newcell/YAK.fixture",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "YAK.fixture": {"package": "root//"},
        "newcell/YAK.fixture": {"package": "newcell//"},
        "nonexistent/file.txt": {"error": "Error listing dir `nonexistent`"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@yak_test()
async def test_audit_file_package_with_errors_plain(yak: Yak) -> None:
    """Test file-package mapping with a mix of valid and invalid paths (plain text)"""
    result = await yak.audit(
        "file-package",
        "YAK.fixture",
        "nonexistent/file.txt",
        "newcell/YAK.fixture",
    )

    # Verify successful paths are in the output with correct format
    assert "YAK.fixture: root//" in result.stdout
    assert "newcell/YAK.fixture: newcell//" in result.stdout

    # Verify error path shows error message
    assert "nonexistent/file.txt: Error:" in result.stdout


@yak_test()
async def test_audit_file_package_absolute_path(yak: Yak) -> None:
    """Test file-package mapping with an absolute path"""
    abs_path = str(yak.cwd / "YAK.fixture")
    result = await yak.audit("file-package", abs_path, "--json")

    data = json.loads(result.stdout)
    expected = {abs_path: {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"
