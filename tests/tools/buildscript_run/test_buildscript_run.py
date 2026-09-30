# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Tests how prelude/rust/tools/buildscript_run.py turns the `cargo:` lines of a
# build script into rustc flags.

import os
import subprocess
import sys
from pathlib import Path

BUILDSCRIPT_RUN: Path = (
    Path(__file__).resolve().parents[3]
    / "prelude"
    / "rust"
    / "tools"
    / "buildscript_run.py"
)


def run_buildscript(tmp_path: Path, lines: list[str]) -> tuple[list[str], str]:
    """Runs a build script that prints `lines`.

    Returns the rustc flags and the linker argument file for dependents.

    `true` stands in for rustc, which the runner only asks for its version.
    """
    script = tmp_path / "build-script"
    script.write_text(
        f"#!{sys.executable}\nfor line in {lines!r}:\n    print(line)\n"
    )
    script.chmod(0o755)
    rustc_cfg = tmp_path / "rustc_cfg"
    rustc_cfg.write_text('unix\ntarget_os="linux"\n')
    manifest_dir = tmp_path / "manifest"
    manifest_dir.mkdir()
    out_dir = tmp_path / "out"
    outfile = tmp_path / "rustc_flags"
    linker_flags = tmp_path / "linker_flags"
    subprocess.run(
        [
            sys.executable,
            str(BUILDSCRIPT_RUN),
            f"--buildscript={script}",
            f"--rustc-cfg={rustc_cfg}",
            f"--manifest-dir={manifest_dir}",
            f"--create-cwd={tmp_path / 'cwd'}",
            f"--outfile={outfile}",
            f"--linker-flags={linker_flags}",
            "--rustc-link-lib",
            "--rustc-link-search",
        ],
        check=True,
        cwd=tmp_path,
        env=dict(
            os.environ,
            OUT_DIR=str(out_dir),
            RUSTC="true",
            TARGET="x86_64-unknown-linux-gnu",
        ),
    )
    return outfile.read_text().splitlines(), linker_flags.read_text()


def test_link_search_outside_out_dir_is_kept(tmp_path: Path) -> None:
    # pkg-config reports a system library this way.
    flags, linker_flags = run_buildscript(
        tmp_path,
        [
            "cargo:rustc-link-search=native=/opt/lib",
            "cargo:rustc-link-search=native=/opt/my libs",
            "cargo:rustc-link-lib=git2",
        ],
    )
    assert flags == ["-Lnative=/opt/lib", "-Lnative=/opt/my libs", "-lgit2"]
    # The links of dependents need the directories too.
    assert linker_flags == '-L/opt/lib\n-L"/opt/my libs"\n'


def test_link_search_in_out_dir_is_relative_to_out_dir(tmp_path: Path) -> None:
    out_dir = (tmp_path / "out").resolve()
    flags, linker_flags = run_buildscript(
        tmp_path,
        [
            f"cargo:rustc-link-search=native={out_dir}/lib",
            "cargo:rustc-link-lib=static=bundled",
        ],
    )
    assert flags == [
        "-Lnative=${__BUILDSCRIPT_OUT_DIR__}/lib",
        "-lstatic=bundled",
    ]
    assert linker_flags == ""


def test_relative_link_search_is_dropped(tmp_path: Path) -> None:
    flags, linker_flags = run_buildscript(
        tmp_path, ["cargo:rustc-link-search=native=lib"]
    )
    assert flags == []
    assert linker_flags == ""
