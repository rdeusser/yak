# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import contextlib
import json
import os
import platform
import shutil
import sys
import tempfile
from collections import namedtuple
from pathlib import Path
from typing import (
    Any,
    AsyncGenerator,
    AsyncIterator,
    Awaitable,
    Callable,
    Dict,
    List,
    Optional,
)

import pytest
from decorator import decorator
from e2e_util.api.buck import Buck
from e2e_util.api.executable import WindowsCmdOption

# The directory that holds `e2e_util`, `core`, and the other test directories.
TESTS_DIR: Path = Path(__file__).resolve().parent.parent

# BUCK2_BINARY names the binary under test. It defaults to the Cargo debug build.
BUCK2_BINARY_ENV_VAR = "BUCK2_BINARY"
# BUCK2_TEST_RE_CONFIG names a buckconfig file with the `[buck2_re_client]`
# settings of a Remote Execution backend. Every test project reads it, and tests
# marked `remote_execution` run only when it is set.
RE_CONFIG_ENV_VAR = "BUCK2_TEST_RE_CONFIG"
# Tests marked `cgroups` run only when BUCK2_TEST_CGROUPS is 1.
CGROUPS_ENV_VAR = "BUCK2_TEST_CGROUPS"

BuckTestMarker = namedtuple(
    "BuckTestMarker",
    [
        "data_dir",
        "allow_soft_errors",
        "extra_buck_config",
        "skip_final_kill",
        "disable_daemon_cgroup",
        "write_invocation_record",
    ],
)


def buck2_binary() -> Path:
    """buck2_binary returns the absolute path of the binary under test."""
    configured = os.environ.get(BUCK2_BINARY_ENV_VAR)
    if configured:
        return Path(configured).resolve()
    exe = "buck2.exe" if platform.system() == "Windows" else "buck2"
    return TESTS_DIR.parent / "target" / "debug" / exe


def test_data_dir(test_file: Path) -> Path:
    """test_data_dir returns the data directory paired with a test module.

    The data for `test_foo.py` lives in `test_foo_data/` next to it.
    """
    return test_file.with_name(test_file.stem + "_data")


@contextlib.asynccontextmanager
async def buck_fixture(  # noqa C901 : "too complex"
    marker: BuckTestMarker,
    test_file: Path,
) -> AsyncGenerator[Buck, None]:
    """Returns a Buck that runs in a new temporary project for the test in `test_file`."""

    binary = buck2_binary()
    if not binary.is_file():
        raise Exception(
            f"buck2 binary `{binary}` does not exist. Build it with "
            f"`cargo build --bin=buck2` or set {BUCK2_BINARY_ENV_VAR}."
        )

    # Remove variables that describe the outer pytest run, so a Python test that
    # buck2 runs inside the test project does not inherit them.
    env: Dict[str, str] = {
        key: value for key, value in os.environ.items() if not key.startswith("PYTEST_")
    }
    # This is necessary for static linking on Linux.
    if platform.system() != "Windows":
        env["BUCKD_STARTUP_TIMEOUT"] = "120"
        env["BUCKD_STARTUP_INIT_TIMEOUT"] = "120"

    env["BUCK2_HARD_ERROR"] = "false" if marker.allow_soft_errors else "true"
    # Use a very small stdin buffer to catch any scenarios in which we
    # don't properly handle partial input.
    env["BUCK2_TEST_STDIN_BUFFER_SIZE"] = "8"
    # Require the events dispatcher to be set for e2e tests.
    env["ENFORCE_DISPATCHER_SET"] = "true"
    # Inform buck of the test timeout
    env["BUCK2_SELF_TEST_TIMEOUT_S"] = "600"
    # Timeout Watchman requests because we often see it hang and crash.
    env["BUCK2_WATCHMAN_TIMEOUT"] = "30"
    # Use few threads. Tests do little work, but many daemons can run at once.
    env["BUCK2_RUNTIME_THREADS"] = "8"
    # Windows uses blocking threads for subprocess I/O, so the blocking pool
    # keeps its default size.
    env.pop("BUCK2_MAX_BLOCKING_THREADS", None)
    # A fixed console size keeps golden files independent of the terminal.
    env["SUPERCONSOLE_TESTING_WIDTH"] = "100"
    env["SUPERCONSOLE_TESTING_HEIGHT"] = "100"
    # clap wraps help text to `COLUMNS`, which pytest sets to 80 while it
    # captures output and leaves unset under `-s`.
    env["COLUMNS"] = "100"
    # The `nano_prelude` bundled cell that most test projects use loads its
    # files from this directory.
    env["NANO_PRELUDE"] = str(TESTS_DIR / "e2e_util" / "nano_prelude")
    # Don't try to assign to a new cgroup during tests.
    if marker.disable_daemon_cgroup:
        env["BUCK2_TEST_DISABLE_DAEMON_CGROUP"] = "true"
    env["BUCK2_TEST_SKIP_DEFAULT_EXTERNAL_CONFIG"] = "true"

    base_dir = Path(tempfile.mkdtemp())
    keep_temp = os.environ.get("BUCK_E2E_KEEP_TEMP") == "1"

    # Keep the daemon directories (`~/.buck/buckd`) of the test inside its
    # temporary directory.
    home_dir = base_dir / "home"
    home_dir.mkdir()
    env["BUCK2_TEST_HOME_DIR"] = str(home_dir)

    # Golden file helpers find the test data through this variable.
    test_data = test_data_dir(test_file)
    os.environ["TEST_REPO_DATA_SRC"] = str(test_data)

    project_dir = base_dir / "project"
    extra_config_lines = []

    orig_stdout = sys.stdout
    try:
        # Redirect stdout to stderr during the test so that `print` statements
        # show up in the test failure output by default for debugging.
        sys.stdout = sys.stderr

        if marker.data_dir is not None:
            src = test_data / marker.data_dir
            if not src.is_dir():
                raise Exception(f"Test data directory `{src}` does not exist")
            _copytree(src, project_dir)
            with open(Path(project_dir, ".watchmanconfig"), "w") as f:
                # Use the FS Events watcher, which is more reliable than the default.
                json.dump(
                    {
                        "ignore_dirs": ["buck-out", ".git", ".hg"],
                        "fsevents_watch_files": True,
                        "prefer_split_fsevents_watcher": False,
                    },
                    f,
                )
        else:
            project_dir.mkdir()

        # The crawler rehashes the project on every command, so each command
        # sees the files a test changed without waiting for file system events.
        extra_config_lines.append("[buck2]\nfile_watcher = fs_hash_crawler\n")

        re_config = os.environ.get(RE_CONFIG_ENV_VAR)
        if re_config:
            extra_config_lines.append(Path(re_config).read_text() + "\n")

        for section, config in marker.extra_buck_config.items():
            extra_config_lines.append(f"[{section}]\n")
            for key, value in config.items():
                extra_config_lines.append(f"{key} = {value}\n")

        extra_config = os.path.join(base_dir, "extra.bcfg")
        with open(extra_config, "w") as f:
            for line in extra_config_lines:
                f.write(line)
        env["BUCK2_TEST_EXTRA_EXTERNAL_CONFIG"] = extra_config

        settings_home_dir = os.path.join(base_dir, "settings_home")
        os.makedirs(settings_home_dir, exist_ok=True)
        env["BUCK2_TEST_SETTINGS_HOME_DIR"] = settings_home_dir

        buck = Buck(
            binary,
            cwd=project_dir,
            encoding="utf-8",
            env=env,
            write_invocation_record=marker.write_invocation_record,
        )

        yield buck

        if not marker.skip_final_kill:
            if keep_temp:
                await buck.kill()
            else:
                await buck.clean()
    finally:
        sys.stdout = orig_stdout
        os.environ.pop("TEST_REPO_DATA_SRC", None)

        if keep_temp:
            print(f"Not deleting temporary directory at {base_dir}", file=sys.stderr)
        else:
            shutil.rmtree(base_dir, ignore_errors=True)


@pytest.fixture(scope="function")
async def buck(request: pytest.FixtureRequest) -> AsyncIterator[Buck]:
    marker = request.node.get_closest_marker("buck_test")
    if marker is None:
        raise Exception(
            "Test method must be decorated with @buck_test() to use the buck fixture."
        )
    async with buck_fixture(marker.args[0], request.path) as buck:
        yield buck


def _copytree(src: Path, dst: Path) -> None:
    """Copies all files and directories from src into dst"""
    dst.mkdir(parents=True, exist_ok=True)
    for item in os.listdir(src):
        if item == "buck-out":
            continue
        s = src / item
        d = dst / item
        if os.path.isdir(s):
            shutil.copytree(s, d, dirs_exist_ok=True)
        else:
            shutil.copy2(s, d)


BuckTestFn = Callable[..., Awaitable[None]]

SKIPPABLE_PLATFORMS = ["darwin", "linux", "windows"]


def buck_test(
    data_dir: Optional[str] = "",
    # Accepted values are specified in SKIPPABLE_PLATFORMS
    skip_for_os: List[str] = [],  # noqa: B006 value is read-only
    allow_soft_errors: bool = False,
    extra_buck_config: Optional[Dict[str, Dict[str, str]]] = None,
    skip_final_kill: bool = False,
    disable_daemon_cgroup: bool = True,
    write_invocation_record: bool = False,
) -> Callable[..., Any]:
    """
    Defines a buck test. This is a must have decorator on all test case functions.

    Each test runs in a new temporary project. `buck_test` copies the test's data
    directory (`test_foo_data/` for `test_foo.py`) into the project.

    Parameters:
        data_dir:
            The subdirectory of the test data directory to copy into the project.
            The default, "", copies the whole data directory, and None starts
            from an empty project.
        skip_for_os:
            List of OS to skip the test on.
        allow_soft_errors:
            Like it says in the arg name. The default is to hard error.
        extra_buck_config:
            A optional dict of extra buck config to add to the test.
            The key is the section name, the value is a dict of key value pairs.
        skip_final_kill:
            Don't run a `buck2 kill` or `buck2 clean` at the end of the test
        disable_daemon_cgroup:
            False lets the daemon move itself into a cgroup with
            `systemd-run --user`, and marks the test `cgroups`.
        write_invocation_record:
            Makes each command write its invocation record, which
            `BuckResult.invocation_record()` reads.
    """

    for p in skip_for_os:
        if p not in SKIPPABLE_PLATFORMS:
            raise Exception(f"skip_for_os must specifiy one of {SKIPPABLE_PLATFORMS}")

    marks = [
        pytest.mark.buck_test(
            BuckTestMarker(
                data_dir=data_dir,
                allow_soft_errors=allow_soft_errors,
                extra_buck_config=extra_buck_config or {},
                skip_final_kill=skip_final_kill,
                disable_daemon_cgroup=disable_daemon_cgroup,
                write_invocation_record=write_invocation_record,
            )
        )
    ]
    if not disable_daemon_cgroup:
        marks.append(pytest.mark.cgroups)
    current_os = platform.system().lower()
    if current_os in skip_for_os:
        marks.append(pytest.mark.skip(reason=f"test does not support {current_os}"))

    def apply_marks(fn: Callable[..., Any]) -> Callable[..., Any]:
        for mark in marks:
            fn = mark(fn)
        return fn

    return apply_marks


def env(key: str, value: str) -> Callable[..., Any]:
    """
    Decorator for adding an environment variable to a test case.
    For example, @env("BUCK_LOG", "info")
    """

    def inner_decorator(fn: BuckTestFn) -> Callable[..., Any]:
        async def wrapped(
            fn: BuckTestFn, buck: Buck, *args: Any, **kwargs: Any
        ) -> None:
            buck.set_env(key, value)
            return await fn(buck, *args, **kwargs)

        return decorator(wrapped, fn)

    return inner_decorator


def windows_cmd_option(key: WindowsCmdOption, value: bool) -> Callable[..., Any]:
    """
    Decorator for specifying the state for cmd.exe's specified key feature
    For example, @windows_cmd_option(WindowsCmdOption.DelayedExpansion, True)
    """

    def inner_decorator(fn: BuckTestFn) -> Callable[..., Any]:
        async def wrapped(
            fn: BuckTestFn, buck: Buck, *args: Any, **kwargs: Any
        ) -> None:
            buck.set_windows_cmd_option(key, value)
            return await fn(buck, *args, **kwargs)

        return decorator(wrapped, fn)

    return inner_decorator
