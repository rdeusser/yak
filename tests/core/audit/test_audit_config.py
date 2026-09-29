# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from pathlib import Path

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test(
    extra_yak_config={
        "test": {
            "foo": "bar",
        }
    },
)
async def test_extra_yak_config(yak: Yak) -> None:
    """
    Assert that our testing framework works as expected.
    """

    cfg = (await yak.audit_config("--style=json")).get_json()
    assert cfg.get("test.foo") == "bar"


@yak_test()
async def test_audit_config_json(yak: Yak) -> None:
    result = await yak.audit_config("--style=json")
    result_json = result.get_json()
    assert result_json is not None


@yak_test()
async def test_audit_config_cell_json(yak: Yak) -> None:
    out = await yak.audit_config(
        "--style",
        "json",
    )
    out_json = out.get_json() or {}
    assert out_json.get("test.is_root") == "yes"
    assert out_json.get("test.is_code") is None

    out = await yak.audit_config("--style", "json", "--cell", "code")
    out_json = out.get_json() or {}
    assert out_json.get("test.is_code") == "yes"
    assert out_json.get("test.is_root") is None

    out = await yak.audit_config(
        "--style",
        "json",
        rel_cwd=Path("code"),
    )
    out_json = out.get_json() or {}
    assert out_json.get("test.is_code") == "yes"
    assert out_json.get("test.is_root") is None


@yak_test()
async def test_audit_config_all_cells(yak: Yak) -> None:
    out = await yak.audit_config(
        "--all-cells",
        "--style",
        "json",
    )
    out_json = out.get_json() or {}
    print(out_json)
    assert out_json.get("code//bar.a") == "2"
    assert out_json.get("source//bar.a") == "1"
    assert out_json.get("root//bar.a") == "1"
    assert out_json.get("b//bar.a") is None

    out = await yak.audit_config(
        "--all-cells",
        "--style",
        "json",
        "code//bar.a",
    )
    out_json = out.get_json() or {}
    assert out_json.get("code//bar.a") == "2"
    assert out_json.get("source//bar.a") is None

    out = await yak.audit_config(
        "--all-cells",
    )
    assert "# Cell: source\n[bar]\n    a = 1\n" in out.stdout


@yak_test()
async def test_audit_config_with_config_value(yak: Yak) -> None:
    result_config = await yak.audit_config(
        "python",
        "--style",
        "json",
        "-cpython.helpers=true",
    )
    result_config_json = result_config.get_json()

    assert result_config_json.get("python.helpers") == "true"


@yak_test()
async def test_audit_config_with_config_file(yak: Yak, tmp_path: Path) -> None:
    configfile = tmp_path / "config.bcfg"
    configfile.write_text("[python]\n  helpers = true\n")

    result_file = await yak.audit_config(
        "--config-file",
        str(configfile),
        "--style",
        "json",
    )

    assert result_file.get_json().get("python.helpers") == "true"


@yak_test()
async def test_audit_config_location_extended(yak: Yak) -> None:
    result = await yak.audit_config(
        "bar.a",
        "--location=extended",
    )
    assert "a = 1" in result.stdout
    assert "included.bcfg:2" in result.stdout


@yak_test()
async def test_audit_config_with_cell_syntax(yak: Yak) -> None:
    result_file = await yak.audit_config(
        "code//test.is_code",
        "--style",
        "json",
    )
    result_file_json = result_file.get_json()

    assert result_file_json.get("code//test.is_code") == "yes"


@yak_test()
async def test_cell_relative_configs(yak: Yak) -> None:
    result_root_cell = await yak.audit_config(
        "--config",
        "root//bar.a=5",
        "--style",
        "json",
    )
    result_root_cell_json = result_root_cell.get_json()

    assert result_root_cell_json is not None
    assert result_root_cell_json.get("foo.b") == "5"

    result_nonroot_cell = await yak.audit_config(
        "foo",
        "--config",
        "code//bar.a=5",
        "--style",
        "json",
        "--cell",
        "code",
    )
    result_nonroot_cell_json = result_nonroot_cell.get_json()

    assert result_nonroot_cell_json is not None
    assert result_nonroot_cell_json.get("foo.b") == "5"

    result_diff_cell = await yak.audit_config(
        "foo",
        "--config",
        "code//bar.a=5",
        "--style",
        "json",
        "--cell",
        "source",
    )
    result_diff_cell_json = result_diff_cell.get_json()

    assert result_diff_cell_json is not None
    assert result_diff_cell_json.get("foo.b") == "1"

    result_all_cell = await yak.audit_config(
        "foo",
        "--config",
        "bar.a=5",
        "--style",
        "json",
        "--cell",
        "source",
    )
    result_all_cell_json = result_all_cell.get_json()

    assert result_all_cell_json is not None
    assert result_all_cell_json.get("foo.b") == "5"
