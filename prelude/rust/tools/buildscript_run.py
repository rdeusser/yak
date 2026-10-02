# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Run a crate's Cargo buildscript.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, IO, NamedTuple, Optional


IS_WINDOWS: bool = os.name == "nt"
TOOL_CWD: str = os.path.join(os.getcwd(), "")

# Sentinel used to mark OUT_DIR-relative paths emitted by buildscripts.
# We later replace this sentinel with the actual content-addressed path, once that is known.
OUT_DIR_SENTINEL: str = "${__BUILDSCRIPT_OUT_DIR__}"


def eprint(*args: Any, **kwargs: Any) -> None:
    print(*args, end="\n", file=sys.stderr, flush=True, **kwargs)


def quote_arg(arg: str) -> str:
    """Quotes an argument of a linker argument file that contains whitespace."""
    if any(c.isspace() for c in arg):
        return '"' + arg.replace("\\", "\\\\").replace('"', '\\"') + '"'
    return arg


SHARED_LIBRARY_PATTERN: re.Pattern[str] = re.compile(r".*(\.dylib|\.dll|\.so(\.[0-9]+)*)$")


def copy_shared_libraries(search_dir: str, shared_libs: Path) -> None:
    """Copies the shared libraries of `search_dir` into `shared_libs`.

    Cargo puts the search paths inside its target directory on the dynamic
    library path of the programs it runs. The binaries that link these
    libraries load them from `shared_libs` instead, whose path is known
    before the build script runs.
    """
    if not os.path.isdir(search_dir):
        return
    for name in sorted(os.listdir(search_dir)):
        path = os.path.join(search_dir, name)
        target = shared_libs / name
        if SHARED_LIBRARY_PATTERN.match(name) and os.path.isfile(path):
            if not target.exists():
                shutil.copy2(path, target)


def cfg_env(rustc_cfg: Path) -> dict[str, str]:
    with rustc_cfg.open(encoding="utf-8") as f:
        lines = f.readlines()

    cfgs: dict[str, str] = {}
    for line in lines:
        if (
            line.startswith("unix")
            or line.startswith("windows")
            or line.startswith("target_")
        ):
            keyval = line.strip().split("=")
            key = keyval[0]
            val = keyval[1].replace('"', "") if len(keyval) > 1 else "1"

            key = "CARGO_CFG_" + key.upper()
            if key in cfgs:
                cfgs[key] = cfgs[key] + "," + val
            else:
                cfgs[key] = val

    return cfgs


def create_cwd(root: Path, manifest_dir: Path, subdir: str) -> list[Path]:
    """Create a tree at root with most of the same contents as manifest_dir, but
    excluding Rustup's rust-toolchain.toml configuration file.

    The entries of the tree are symlinks into manifest_dir, except for the
    directories on the way to `subdir`, where the build script runs. A relative
    path out of the script's directory, such as `../proto/api.proto`, resolves
    as it does from `subdir` of manifest_dir.

    Returns the symlinks it created, which `remove_cwd_links` removes after the
    build script exits.

    Keeping rust-toolchain.toml goes wrong in the situation that all of the
    following happen:

      1. toolchains//:rust uses compiler = "rustc", like the
         system_rust_toolchain.

      2. The rustc in $PATH is rustup's rustc shim.

      3. A third-party dependency has both a rust-toolchain.toml and a build.rs
         that runs "rustc" or env::var_os("RUSTC"), such as to inspect `rustc
         --version` or to compile autocfg-style probe code.

    Cargo defines that build scripts run using the package's manifest directory
    as the current directory, so the rustc subprocess spawned from build.rs
    would also run in that manifest directory. But other rustc invocations
    performed by yak run from the repo root.

    Rustup only looks at one rust-toolchain.toml file, using the nearest one
    present in any parent directory. The file can set `channel` to control which
    installed version of rustc to run.

    It is bad if it's possible for the rustc run by a build script vs rustc run
    by the rest of the build to be different toolchains. In order to configure
    their crate appropriately, build scripts rely on using the same rustc that
    their crate will be later compiled by.

    This problem doesn't happen during Cargo-based builds because rustup
    installs both a cargo shim and a rustc shim. When you run a rustup-managed
    Cargo, one of the first things it does is define a RUSTUP_TOOLCHAIN
    environment variable pointing to the rustup channel id of the currently
    selected cargo. Subsequent invocations of the rustup cargo shim or rustc
    shim with this variable in the environment no longer pay attention to any
    rust-toolchain.toml file.

    We cannot follow the same approach because there is no API in rustup for
    finding out a suitable RUSTUP_TOOLCHAIN value consistent with which
    toolchain "rustc" currently refers to, and even if there were, it isn't
    guaranteed that "rustc" refers to a rustup-managed toolchain in the first
    place.
    """

    parts = [part for part in subdir.split("/") if part]
    links = []
    tree_dir = root
    source_dir = manifest_dir
    for depth in range(len(parts) + 1):
        tree_dir.mkdir(parents=True, exist_ok=True)
        next_part = parts[depth] if depth < len(parts) else None
        for dir_entry in source_dir.iterdir():
            if dir_entry.name in ["rust-toolchain", "rust-toolchain.toml", next_part]:
                continue
            link = tree_dir.joinpath(dir_entry.name)
            link.unlink(missing_ok=True)
            link.symlink_to(
                os.path.relpath(dir_entry, tree_dir),
                target_is_directory=dir_entry.is_dir(),
            )
            links.append(link)
        if next_part is not None:
            tree_dir = tree_dir.joinpath(next_part)
            source_dir = source_dir.joinpath(next_part)

    return links


def remove_cwd_links(links: list[Path]) -> None:
    """Remove the symlinks that `create_cwd` created.

    The symlinks point into the manifest directory, which is an input of the
    action. The local action cache does not persist an output that holds
    symlinks to inputs, so leaving them would make every build script run again
    after the daemon restarts. Files that the build script wrote to its current
    directory stay.
    """
    for link in links:
        if link.is_symlink():
            link.unlink()


# In some environments, invoking the rustc binary may actually invoke another
# tool that fetches the binary from a remote location. This fetch may encounter
# network errors. Ideally, build scripts that invoke rustc would reliably fail
# when such a thing happens, but in practice they don't. To mitigate, we
# manually invoke `rustc --version` and make sure that succeeds.
def ensure_rustc_available(
    env: dict[str, str],
    cwd: Path,
    target: str,
) -> None:
    rustc = env.get("RUSTC")
    assert rustc is not None, "RUSTC env is missing"

    # NOTE: `HOST` is optional.
    host = env.get("HOST")

    try:
        # Run through cmd.exe on Windows so if rustc is a batch script
        # (like the command_alias trampoline is), it is found relative to
        # cwd.
        #
        # Executing `os.path.join(cwd, rustc)` would also work, but because
        # of `../` in the path, it's possible to hit path length limits.
        # Resolving it would remove the `..` but then sometimes things
        # fail with exit code `3221225725` ("out of stack memory").
        # I suspect it's some infinite loop brought about by the trampoline
        # and symlinks.
        subprocess.check_output(  # noqa: P204
            [rustc, "--version"],
            cwd=cwd,
            shell=IS_WINDOWS,
        )
        # A multiplexed sysroot may involve another fetch,
        # so pass `--target` to check that too.
        if host != target:
            subprocess.check_output(  # noqa: P204
                [rustc, f"--target={target}", "--version"],
                cwd=cwd,
                shell=IS_WINDOWS,
            )
    except OSError as ex:
        eprint(f"Failed to run {rustc} because {ex}")
        sys.exit(1)
    except subprocess.CalledProcessError as ex:
        eprint(f"Command failed with exit code {ex.returncode}")
        eprint(f"Command: {ex.cmd}")
        if ex.stdout:
            eprint(f"Stdout: {ex.stdout}")
        sys.exit(1)


def run_buildscript(
    buildscript: str,
    env: dict[str, str],
    cwd: Path,
) -> str:
    try:
        return subprocess.check_output(
            os.path.abspath(buildscript),
            encoding="utf-8",
            env=env,
            cwd=cwd,
        )
    except OSError as ex:
        print(f"Failed to run {buildscript} because {ex}", file=sys.stderr)
        sys.exit(1)
    except subprocess.CalledProcessError as ex:
        # A failing script can report why on stdout, as `cargo::error=` lines
        # do, so show it as Cargo does.
        if ex.stdout:
            eprint(f"--- stdout of {buildscript}\n{ex.stdout}")
        sys.exit(ex.returncode)


class Args(NamedTuple):
    buildscript: str
    rustc_cfg: Path
    rustc_host_tuple: Optional[Path]
    manifest_dir: Path
    manifest_subdir: str
    create_cwd: Path
    outfile: IO[str]
    linker_flags: Optional[IO[str]]
    linker_search_flag: str
    shared_libs: Optional[Path]
    rustc_link_lib: bool
    rustc_link_search: bool


def arg_parse() -> Args:
    parser = argparse.ArgumentParser(description="Run Rust build script")
    parser.add_argument("--buildscript", type=str, required=True)
    parser.add_argument("--rustc-cfg", type=Path, required=True)
    parser.add_argument("--rustc-host-tuple", type=Path)
    parser.add_argument("--manifest-dir", type=Path, required=True)
    # The directory of the package in the tree of `--manifest-dir`, which
    # `--create-cwd` names in the tree that it creates.
    parser.add_argument("--manifest-subdir", type=str, default="")
    parser.add_argument("--create-cwd", type=Path, required=True)
    parser.add_argument("--outfile", type=argparse.FileType("w"), required=True)
    # The linker flags that every link of a dependent needs, as an argument file.
    parser.add_argument("--linker-flags", type=argparse.FileType("w"))
    parser.add_argument("--linker-search-flag", type=str, default="-L")
    # The directory that receives the shared libraries of the search paths in
    # `OUT_DIR`.
    parser.add_argument("--shared-libs", type=Path)
    parser.add_argument("--rustc-link-lib", action="store_true")
    parser.add_argument("--rustc-link-search", action="store_true")

    return Args(**vars(parser.parse_args()))


def main() -> None:  # noqa: C901
    args = arg_parse()

    env = cfg_env(args.rustc_cfg)

    out_dir = os.getenv("OUT_DIR")
    assert out_dir is not None, "OUT_DIR env is missing"
    os.makedirs(out_dir, exist_ok=True)
    env["OUT_DIR"] = os.path.abspath(out_dir)

    cwd = args.create_cwd
    cwd_root = cwd
    for part in args.manifest_subdir.split("/"):
        if part:
            cwd_root = cwd_root.parent
    cwd_links = create_cwd(cwd_root, args.manifest_dir, args.manifest_subdir)
    cwd_abs = os.path.abspath(cwd)
    cwd_root_abs = os.path.abspath(cwd_root)
    env["CARGO_MANIFEST_DIR"] = cwd_abs

    env = dict(os.environ, **env)

    target = env.get("TARGET")
    if target is None:
        assert args.rustc_host_tuple, "TARGET env is missing"
        with args.rustc_host_tuple.open(encoding="utf-8") as f:
            target = f.read().strip()
            env["TARGET"] = target

    if os.sep in env.get("LD", ""):
        env["LD"] = os.path.abspath(env["LD"])
    if os.sep in env.get("CC", ""):
        env["CC"] = os.path.abspath(env["CC"])
    if os.sep in env.get("CXX", ""):
        env["CXX"] = os.path.abspath(env["CXX"])
    if os.sep in env.get("AR", ""):
        env["AR"] = os.path.abspath(env["AR"])

    ensure_rustc_available(
        env=env,
        cwd=cwd,
        target=target,
    )

    script_output = run_buildscript(args.buildscript, env=env, cwd=cwd)

    cargo_rustc_cfg_pattern = re.compile("^cargo::?rustc-cfg=(.*)")
    cargo_rustc_env_pattern = re.compile("^cargo::?rustc-env=(.+?)=(.*)")
    cargo_rustc_link_lib_pattern = re.compile("^cargo::?rustc-link-lib=(.*)")
    cargo_rustc_link_search_pattern = re.compile(
        "^cargo::?rustc-link-search=([a-z]+=)?(.+)"
    )
    out_dir_abs = env["OUT_DIR"]

    # Resolve a path inside the tree of the build script's current directory
    # through the symlinks that `remove_cwd_links` removes.
    def resolve_cwd(path: str) -> str:
        if path == cwd_abs:
            return os.path.abspath(args.manifest_dir.joinpath(args.manifest_subdir))
        if path.startswith(cwd_root_abs + os.sep):
            return os.path.realpath(path)
        return path

    # Rewrite a path inside OUT_DIR to OUT_DIR_SENTINEL; None if it is elsewhere.
    def reanchor_out_dir(path: str) -> Optional[str]:
        if path == out_dir_abs:
            return OUT_DIR_SENTINEL
        if path.startswith(out_dir_abs + os.sep):
            return OUT_DIR_SENTINEL + path[len(out_dir_abs) :]
        return None

    flags = ""
    linker_flags = ""
    if args.shared_libs:
        args.shared_libs.mkdir(parents=True, exist_ok=True)
    for line in script_output.split("\n"):
        cargo_rustc_cfg_match = cargo_rustc_cfg_pattern.match(line)
        if cargo_rustc_cfg_match:
            value = cargo_rustc_cfg_match.group(1)
            flags += f"--cfg={value}\n"
            continue
        cargo_rustc_env_match = cargo_rustc_env_pattern.match(line)
        if cargo_rustc_env_match:
            key = cargo_rustc_env_match.group(1)
            value = resolve_cwd(cargo_rustc_env_match.group(2))
            reanchored = reanchor_out_dir(value)
            if reanchored is not None:
                flags += f"--env-set={key}={reanchored}\n"
            elif value.startswith(TOOL_CWD):
                relative_path = value[len(TOOL_CWD) :]
                flags += f"--env-set={key}=$(abspath {relative_path})\n"
            else:
                flags += f"--env-set={key}={value}\n"
            continue
        cargo_rustc_link_lib_match = cargo_rustc_link_lib_pattern.match(line)
        if args.rustc_link_lib and cargo_rustc_link_lib_match:
            value = cargo_rustc_link_lib_match.group(1)
            flags += f"-l{value}\n"
            continue
        cargo_rustc_link_search_match = cargo_rustc_link_search_pattern.match(line)
        if args.rustc_link_search and cargo_rustc_link_search_match:
            kind = cargo_rustc_link_search_match.group(1) or ""
            path = resolve_cwd(cargo_rustc_link_search_match.group(2))
            reanchored = reanchor_out_dir(path)
            if reanchored is not None:
                flags += f"-L{kind}{reanchored}\n"
                if args.shared_libs:
                    copy_shared_libraries(path, args.shared_libs)
            elif path.startswith(TOOL_CWD):
                relative_path = path[len(TOOL_CWD) :]
                flags += f"-L{kind}$(abspath {relative_path})\n"
            elif os.path.isabs(path):
                # A directory of the host, such as the one where pkg-config
                # found a system library. rustc records the script's
                # `rustc-link-lib` for the links of dependents, but not the
                # search path, so dependents get it through `--linker-flags`.
                flags += f"-L{kind}{path}\n"
                linker_flags += f"{args.linker_search_flag}{quote_arg(path)}\n"
            continue
        print(line, end="\n")
    remove_cwd_links(cwd_links)
    args.outfile.write(flags)
    if args.linker_flags:
        args.linker_flags.write(linker_flags)


if __name__ == "__main__":
    main()
