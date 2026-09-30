# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import uuid
from asyncio import subprocess
from pathlib import Path
from typing import Dict, Optional, Tuple

from e2e_util.api.yak_result import (
    AuditConfigResult,
    YakException,
    YakResult,
    BuildResult,
    TargetsResult,
)
from e2e_util.api.executable import Executable
from e2e_util.api.lsp import LspClient
from e2e_util.api.process import Process
from e2e_util.api.result import R, Result
from e2e_util.api.subscribe import SubscribeClient


class Yak(Executable):
    """Instantiates a yak object with a executable path"""

    def __init__(
        self,
        path_to_executable: Path,
        encoding: str,
        env: Dict[str, str],
        cwd: Optional[Path] = None,
        write_invocation_record: bool = False,
    ) -> None:
        super().__init__(path_to_executable, encoding, env, cwd)
        self.write_invocation_record = write_invocation_record
        self.isolation_prefix: Optional[str] = None

    def set_isolation_prefix(self, isolation_prefix: str) -> None:
        """Runs later commands with `--isolation-dir <isolation_prefix>`."""
        self.isolation_prefix = isolation_prefix

    def _get_cwd(self, rel_cwd: Optional[Path]) -> Path:
        if rel_cwd is None:
            return self.cwd
        abs_cwd = self.cwd / rel_cwd
        assert abs_cwd.exists(), f"{abs_cwd} doesn't exist"
        return abs_cwd

    def build(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
        stdin: Optional[int] = None,
    ) -> Process[BuildResult, YakException]:
        """
        Returns a Process with BuildResult type using a process
        created with the build command and any
        additional arguments.

        rel_cwd: Optional Path specifying the working directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        args = list(argv)
        if not any(arg.startswith("--build-report") for arg in args):
            # For `build` commands, anything after `--` is a positional arg.
            # Find the position of "--" separator to insert --build-report before it
            separator_idx = args.index("--") if "--" in args else len(args)
            args.insert(separator_idx, "--build-report=-")

        return self._run_yak_command(
            "build",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
            result_type=BuildResult,
            stdin=stdin,
        )

    def build_without_report(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the build command and any
        additional arguments.

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """

        return self._run_yak_command(
            "build",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def help(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "help",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def help_env(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "help-env",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def run(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the build command and any
        additional arguments

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "run",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def clean(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the clean command and any
        additional arguments

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "clean",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def root(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the root command

        kind: --kind argument to the root command
        rel_cwd: Optional Path specifying the workding directory to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "root",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def kill(
        self,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the kill command

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "kill",
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def test(
        self,
        *argv: str,
        test_executor: Optional[str] = None,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the test command and any
        additional arguments

        test_executor: Optional test executor to use in place of the one the
        project configures. An empty string selects the built-in executor.
        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        argv_list = list(argv)
        argv_separator_idx = (
            argv_list.index("--") if "--" in argv_list else len(argv_list)
        )
        yak_argv = argv_list[0:argv_separator_idx]
        test_argv = argv_list[argv_separator_idx + 1 :]

        if test_executor is not None:
            yak_argv = [
                "--config",
                "test.v2_test_executor={}".format(test_executor),
                *yak_argv,
            ]

        patched_argv = yak_argv + ["--"] + test_argv

        return self._run_yak_command(
            "test",
            *patched_argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def targets(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[TargetsResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the targets command and any
        additional arguments

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.

        TODO: Add a TargetsResult with structured output.
        """

        return self._run_yak_command(
            "targets",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
            result_type=TargetsResult,
        )

    def ctargets(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "ctargets",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def complete(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the complete command and any
        additional arguments.

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """

        my_env = {} if env is None else env.copy()
        my_env["YAK_COMPLETION_TIMEOUT"] = "30000"

        return self._run_yak_command(
            "complete",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=my_env,
        )

    def completion(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the completion command and any
        additional arguments.

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "completion",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def audit_config(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[AuditConfigResult, YakException]:
        """
        Returns a Process with AuditConfigResult type using a process
        created with the audit_config command

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        args = list(argv)
        return self._run_yak_command(
            "audit",
            "config",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
            result_type=AuditConfigResult,
        )

    def audit_configurations(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "audit",
            "configurations",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def audit_dep_files(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "audit",
            "dep-files",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def audit_visibility(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "audit",
            "visibility",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def audit(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "audit",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def audit_output(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "audit",
            "output",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def query(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._query("query", *argv, rel_cwd=rel_cwd, env=env)

    def cquery(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._query("cquery", *argv, rel_cwd=rel_cwd, env=env)

    def uquery(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._query("uquery", *argv, rel_cwd=rel_cwd, env=env)

    def aquery(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._query("aquery", *argv, rel_cwd=rel_cwd, env=env)

    def _query(
        self,
        query_command: str,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process
        created with the query command and any
        additional arguments

        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.

        TODO: Add a QueryResult with structured output.
        """
        return self._run_yak_command(
            query_command,
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def bxl(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        args = list(argv)
        return self._run_yak_command(
            "bxl",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def docs(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "docs",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def profile(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process created with the
        profile command and any additional arguments

        args: Arguments to pass to yak profile.
        rel_cwd: Optional Path specifying the workding directive to run
        the command relative to the root.
        env: Optional dictionary for environment variables to run command with.
        """
        return self._run_yak_command(
            "profile",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def debug(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process created with the
        debug command and any additional arguments
        """
        return self._run_yak_command(
            "debug",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def starlark(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        """
        Returns a Process with YakResult type using a process created with the
        debug command and any additional arguments
        """
        return self._run_yak_command(
            "starlark",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def install(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "install",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def log(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "log",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
            can_write_invocation_record=False,
        )

    def status(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "status",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
            can_write_invocation_record=False,
        )

    def server(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "server",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def expand_external_cell(
        self,
        *args: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "expand-external-cell",
            *args,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    async def lsp(
        self,
        *args: str,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> LspClient:
        process = await self._run_yak_command(
            "lsp",
            *args,
            input=None,
            stdin=subprocess.PIPE,
            rel_cwd=rel_cwd,
            env=env,
            intercept_stderr=False,
        ).start()
        cwd = self._get_cwd(rel_cwd)
        return LspClient(process, cwd)

    async def subscribe(
        self,
        *args: str,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> SubscribeClient:
        process = self._run_yak_command(
            "subscribe",
            "--unstable-json",
            *args,
            input=None,
            stdin=subprocess.PIPE,
            rel_cwd=rel_cwd,
            env=env,
            intercept_stderr=False,
        )
        client = await SubscribeClient.create(process)
        return client

    def construct_yak_command(
        self,
        cmd: str,
        *argv: str,
    ) -> list[str]:
        """
        Returns a list of strings representing the yak command
        """
        cmd_to_run = [str(self.path_to_executable)]
        if self.isolation_prefix:
            cmd_to_run += ["--isolation-dir", self.isolation_prefix]
        cmd_to_run.append(cmd)
        cmd_to_run.extend(argv)
        cmd_to_run = self._get_windows_cmd_options() + cmd_to_run
        return cmd_to_run

    def _run_yak_command(
        self,
        cmd: str,
        *argv: str,
        input: Optional[bytes],
        rel_cwd: Optional[Path],
        env: Optional[Dict[str, str]],
        result_type: type[R] = YakResult,
        stdin: Optional[int] = None,
        intercept_stderr: bool = True,
        can_write_invocation_record: bool = True,
    ) -> Process[R, YakException]:
        """
        Returns a process created from the execuable path,
        command and any additional arguments
        """
        yak_build_id = str(uuid.uuid1())
        command_env = self._get_command_env(env)
        if "YAK_WRAPPER_UUID" not in command_env:
            command_env["YAK_WRAPPER_UUID"] = yak_build_id

        cwd = self._get_cwd(rel_cwd)

        args = list(argv)
        invocation_record_path = None
        if self.write_invocation_record and can_write_invocation_record:
            invocation_record_dir = cwd / "yak-out" / "tmp"
            invocation_record_dir.mkdir(parents=True, exist_ok=True)
            invocation_record_path = invocation_record_dir / (yak_build_id + ".json")
            separator_idx = args.index("--") if "--" in args else len(args)
            args[separator_idx:separator_idx] = [
                "--unstable-write-invocation-record",
                str(invocation_record_path),
            ]

        cmd_to_run = self.construct_yak_command(cmd, *args)

        args_tuple: Tuple[str, ...] = argv
        _yak_build_id: str = yak_build_id
        _invocation_record_path: Optional[Path] = invocation_record_path

        def make_result(proc: subprocess.Process, stdout: str, stderr: str) -> R:
            base = YakResult(
                proc,
                stdout,
                stderr,
                _yak_build_id,
                _invocation_record_path,
                yak_args=" ".join(args_tuple),
            )
            if result_type is YakResult:
                return base  # type: ignore[return-value]
            return result_type(base)  # type: ignore[return-value]

        _yak_build_id2: str = yak_build_id
        _invocation_record_path2: Optional[Path] = invocation_record_path

        def make_exception(
            cmd_to_run: str,
            working_dir: Path,
            env: Dict[str, str],
            proc: subprocess.Process,
            stdout: str,
            stderr: str,
        ) -> YakException:
            return YakException(
                cmd_to_run,
                working_dir,
                env,
                proc,
                stdout,
                stderr,
                _yak_build_id2,
                _invocation_record_path2,
            )

        stderr = subprocess.PIPE if intercept_stderr else None
        return Process(
            cmd_to_run=cmd_to_run,
            working_dir=cwd,
            env=command_env,
            input=input,
            stdin=stdin,
            stdout=subprocess.PIPE,
            stderr=stderr,
            result_type=make_result,
            exception_type=make_exception,
            encoding=self.encoding,
        )

    def run_yak_command(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def execute(
        self,
        *argv: str,
        env: Optional[Dict[str, str]] = None,
        input: Optional[bytes] = None,
        stdin: Optional[int] = None,
        stdout: int = subprocess.PIPE,
        stderr: int = subprocess.PIPE,
    ) -> Process[Result, Exception]:
        raise NotImplementedError("yak does not use execute.")

    def generate(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "generate",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    def init(
        self,
        *argv: str,
        input: Optional[bytes] = None,
        rel_cwd: Optional[Path] = None,
        env: Optional[Dict[str, str]] = None,
    ) -> Process[YakResult, YakException]:
        return self._run_yak_command(
            "init",
            *argv,
            input=input,
            rel_cwd=rel_cwd,
            env=env,
        )

    async def get_daemon_dir(self) -> Path:
        return Path((await self.debug("daemon-dir")).stdout.strip())

    async def daemon_stderr(self) -> str:
        daemon_dir = await self.get_daemon_dir()
        return (daemon_dir / "yakd.stderr").read_text()

    async def prev_daemon_stderr(self) -> str:
        daemon_dir = await self.get_daemon_dir()
        return (daemon_dir / "prev/yakd.stderr").read_text()

    def get_settings_home_dir(self) -> Path:
        return Path(self._env["YAK_TEST_SETTINGS_HOME_DIR"])
