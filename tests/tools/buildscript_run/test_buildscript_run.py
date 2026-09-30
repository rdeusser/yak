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


def run_buildscript(
    tmp_path: Path, lines: list[str], manifest_files: tuple[str, ...] = ()
) -> tuple[list[str], str]:
    """Runs a build script that prints `lines`, with `$CARGO_MANIFEST_DIR`
    replaced by the directory the runner gives it.

    The manifest directory holds `manifest_files`, and the script writes
    `written.txt` to its current directory.

    Returns the rustc flags and the linker argument file for dependents.

    `true` stands in for rustc, which the runner only asks for its version.
    """
    script = tmp_path / "build-script"
    script.write_text(
        f"#!{sys.executable}\n"
        "import os\n"
        "open('written.txt', 'w').close()\n"
        f"for line in {lines!r}:\n"
        "    print(line.replace('$CARGO_MANIFEST_DIR', os.environ['CARGO_MANIFEST_DIR']))\n"
    )
    script.chmod(0o755)
    rustc_cfg = tmp_path / "rustc_cfg"
    rustc_cfg.write_text('unix\ntarget_os="linux"\n')
    manifest_dir = tmp_path / "manifest"
    manifest_dir.mkdir()
    for name in manifest_files:
        path = manifest_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(name)
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
            f"--shared-libs={tmp_path / 'shared_libs'}",
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


def test_shared_libraries_in_out_dir_are_copied(tmp_path: Path) -> None:
    lib_dir = tmp_path / "out" / "lib"
    lib_dir.mkdir(parents=True)
    for name in ["libseven.dylib", "libseven.so.1", "seven.dll", "libseven.a"]:
        (lib_dir / name).write_text(name)
    flags, linker_flags = run_buildscript(
        tmp_path,
        [
            f"cargo:rustc-link-search=native={lib_dir.resolve()}",
            "cargo:rustc-link-lib=dylib=seven",
        ],
    )
    assert flags == ["-Lnative=${__BUILDSCRIPT_OUT_DIR__}/lib", "-ldylib=seven"]
    # Binaries that link the library load it from `shared_libs`, and a static
    # library is part of the Rust library that links it.
    assert sorted(p.name for p in (tmp_path / "shared_libs").iterdir()) == [
        "libseven.dylib",
        "libseven.so.1",
        "seven.dll",
    ]
    assert linker_flags == ""


def test_paths_in_the_current_directory_resolve_to_the_manifest_directory(
    tmp_path: Path,
) -> None:
    manifest_dir = (tmp_path / "manifest").resolve()
    flags, linker_flags = run_buildscript(
        tmp_path,
        [
            "cargo:rustc-env=MANIFEST=$CARGO_MANIFEST_DIR",
            "cargo:rustc-env=DATA=$CARGO_MANIFEST_DIR/data/table.txt",
            "cargo:rustc-link-search=native=$CARGO_MANIFEST_DIR/lib",
        ],
        manifest_files=("data/table.txt", "lib/libprebuilt.a"),
    )
    manifest = os.path.relpath(manifest_dir, tmp_path.resolve())
    assert flags == [
        f"--env-set=MANIFEST=$(abspath {manifest})",
        f"--env-set=DATA=$(abspath {manifest}/data/table.txt)",
        f"-Lnative=$(abspath {manifest}/lib)",
    ]
    assert linker_flags == ""


def test_current_directory_keeps_no_symlinks(tmp_path: Path) -> None:
    run_buildscript(tmp_path, [], manifest_files=("src/lib.rs", "Cargo.toml"))
    # The local action cache persists only outputs without symlinks to inputs.
    assert [p.name for p in (tmp_path / "cwd").iterdir()] == ["written.txt"]
