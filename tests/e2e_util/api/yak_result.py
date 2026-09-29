# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import json
import textwrap
from asyncio import subprocess
from collections import defaultdict
from enum import Enum
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Tuple

from e2e_util.api.result import Result


class ExitCode(Enum):
    """The exit codes of yak."""

    SUCCESS = 0
    UNKNOWN_ERROR = 1
    INFRA_ERROR = 2
    USER_ERROR = 3
    DAEMON_IS_BUSY = 4
    DAEMON_PREEMPTED = 5
    TIMEOUT = 6
    CONNECT_ERROR = 11
    BROKEN_PIPE = 130
    SIGNAL_INTERRUPT = 141


class InvocationRecord:
    """Parsed invocation record from a yak command."""

    def __init__(self, path: Path) -> None:
        record_json = json.loads(path.read_text(encoding="utf-8"))
        self._data = record_json["data"]["Record"]["data"]["InvocationRecord"]
        self._data["trace_id"] = record_json["trace_id"]

    def __getitem__(self, key: str) -> Any:
        return self._data[key]

    def __contains__(self, key: str) -> bool:
        return key in self._data

    def get(self, key: str, default: Any = None) -> Any:
        return self._data.get(key, default)

    def single_error(self) -> Dict[str, Any]:
        errors = self._data["errors"]
        assert len(errors) == 1
        return errors[0]


class YakResult(Result):
    """
    Represents a yak process that has finished running and succeeded.
    If the yak process failed, proceeds to raise YakException
    """

    def __init__(
        self,
        process: subprocess.Process,
        stdout: str,
        stderr: str,
        yak_build_id: str,
        invocation_record_path: Optional[Path] = None,
        yak_args: str = "",
    ) -> None:
        super().__init__(process, stdout, stderr)
        self.yak_build_id = yak_build_id
        self.yak_args = yak_args
        self.invocation_record_path = invocation_record_path

    def invocation_record(self) -> InvocationRecord:
        if self.invocation_record_path is None:
            raise Exception(
                "No invocation record available, write_invocation_record not set."
            )
        return InvocationRecord(self.invocation_record_path)


class YakException(Exception, YakResult):
    """Represents a yak process that has finished running and failed."""

    def __init__(
        self,
        cmd_to_run: Iterable[str],
        working_dir: Path,
        env: Dict[str, str],
        process: subprocess.Process,
        stdout: str,
        stderr: str,
        yak_build_id: str,
        invocation_record_path: Optional[Path] = None,
    ) -> None:
        cmd = " ".join(str(e) for e in cmd_to_run)
        if stdout != "":
            indented_stdout = textwrap.indent(stdout, " " * 8)
            rendered_stdout = "\n<stdout>\n" + indented_stdout + "\n</stdout>"
        else:
            rendered_stdout = ""
        rendered_stderr = (
            "\n<stderr>\n" + textwrap.indent(stderr, " " * 8) + "\n</stderr>"
        )
        error_msg = (
            textwrap.dedent(
                f"""
            <cmd> {cmd}
            <working_dir> {working_dir}
            """
            )
            + rendered_stdout
            + rendered_stderr
        )
        Exception.__init__(
            self,
            error_msg,
        )
        YakResult.__init__(
            self, process, stdout, stderr, yak_build_id, invocation_record_path
        )

    def check_returncode(self) -> None:
        assert self.process.returncode != 0

    def get_exit_code(self) -> ExitCode:
        """Returns the exit code of a yak Result when it exits"""
        # See https://docs.python.org/3/library/subprocess.html#subprocess.Popen.returncode
        # for negative return code.
        assert self.process.returncode is not None
        if self.process.returncode < 0:  # type: ignore
            return ExitCode(128 - self.process.returncode)  # type: ignore
        return ExitCode(self.process.returncode)


class BuildReport:
    """
    A parsed build report, which the --build-report flag writes.

    Attributes:
        build_report: A JSON dictionary parsed representation of the build report.
        root: A Path to the project root. Parsed from build report.
        results: A dictionary mapping targets to their build report entries.
    """

    def __init__(self, parsed) -> None:
        assert isinstance(parsed, Dict)
        self.build_report: Dict[str, Any] = parsed  # type: ignore
        self.root = Path(self.build_report["project_root"])  # type: ignore
        self.results: Dict[str, Dict[str, Any]] = self.build_report["results"]

    def _to_abs_paths(self, paths: Tuple[Path, ...]) -> Tuple[Path, ...]:
        return tuple(self.root / path for path in paths)

    @staticmethod
    def _configured_outputs(
        target: str, entry: Dict[str, Any], sub_target: str
    ) -> Tuple[Path, ...]:
        outputs = [
            configured["outputs"][sub_target]
            for configured in entry["configured"].values()
            if sub_target in configured["outputs"]
        ]
        assert len(outputs) == 1, (
            f"Expected outputs of {target}[{sub_target}] in one configuration, found {len(outputs)}: {entry}"
        )
        return outputs[0]

    def outputs_for_target(
        self, target: str, sub_target: str = "DEFAULT", rel_path: bool = False
    ) -> Tuple[Path, ...]:
        assert "//" in target
        paths: Tuple[Path, ...]
        if target.startswith("//"):
            # Get the full target "cell//target" that matches this target.
            matched_outputs = [
                self._configured_outputs(t, entry, sub_target)
                for t, entry in self.results.items()
                if t.endswith(target)
            ]
            assert len(matched_outputs) > 0, (
                f"Found no match for target {target} in {self.build_report}"
            )
            assert len(matched_outputs) == 1, (
                f"Found different cells for target {target} in {self.results}"
            )
            paths = matched_outputs[0]
        else:
            paths = self._configured_outputs(target, self.results[target], sub_target)
        if rel_path:
            return tuple(Path(p) for p in paths)
        return self._to_abs_paths(paths)

    def output_for_target(
        self, target: str, sub_target: str = "DEFAULT", rel_path: bool = False
    ) -> Path:
        paths = self.outputs_for_target(target, sub_target, rel_path)
        assert len(paths) == 1, f"Found more than 1 output for target {target}: {paths}"
        return paths[0]


LOG_COMPUTE_KEY = "build_api::actions::calculation: compute"


class TargetsResult(YakResult):
    """Represents a yak process of a targets command that has finished running"""

    def __init__(self, base: YakResult) -> None:
        self.__dict__.update(base.__dict__)

    def get_target_list(self) -> List[str]:
        """
        Returns a list of sorted target labels
        """
        assert "--json-lines" in self.yak_args, (
            "Must add --json-lines arg to get targets"
        )
        targets = []
        for line in self.stdout.splitlines():
            js = json.loads(line)
            targets.append(js["yak.package"] + ":" + js["name"])
        return sorted(targets)

    def get_target_to_build_output(self) -> Dict[str, str]:
        """
        Returns a dict of the target and its output file in yak-out
        """
        target_to_output = {}
        assert (
            "--show-output" in self.yak_args or "--show-full-output" in self.yak_args
        ), "Must add --show-output or --show-full-output arg to get targets output"
        show_output = self.stdout.strip().splitlines()
        for line in show_output:
            output_mapping = line.split()
            assert len(output_mapping) <= 2, "Output mapping should be less than 2"
            target = output_mapping[0]
            if len(output_mapping) == 1:
                target_to_output[target] = ""
            else:
                target_to_output[target] = output_mapping[1]
        return target_to_output


class BuildResult(YakResult):
    """Represents a yak process of a build command that has finished running"""

    def __init__(self, base: YakResult) -> None:
        self.__dict__.update(base.__dict__)

    def get_target_to_build_output(self) -> Dict[str, str]:
        """
        Returns a dict of the build target and file created in yak-out
        Prints to build target followed by path to yak-out file to stdout
        """
        target_to_output = {}
        assert (
            "--show-output" in self.yak_args or "--show-full-output" in self.yak_args
        ), "Must add --show-output or --show-full-output arg to get build output"
        show_output = self.stdout.strip().splitlines()
        if "--build-report=-" in self.yak_args:
            # When mixing --show-output with --build-report=-, the first line is
            # the build report, and the remaining ones are the results, we only
            # want the results for the purpose of this function so we skip the report
            show_output = show_output[1:]
        for line in show_output:
            output_mapping = line.split()
            assert len(output_mapping) <= 2, "Output mapping should be less than 2"
            target = output_mapping[0]
            if len(output_mapping) == 1:
                target_to_output[target] = ""
            else:
                target_to_output[target] = output_mapping[1]
        return target_to_output

    def get_build_report(self) -> BuildReport:
        """
        Returns a BuildReport object for a yak build invoked with --build-report.
        Looks for a '{' and parses the build stdout starting from '{' as a json.
        """
        try:
            start = self.stdout.index("{")
            end = self.stdout.index("\n", start)
            parsed = json.loads(self.stdout[start:end])
            return BuildReport(parsed)
        except Exception as e:
            print(f"stdout: {self.stdout}\nstderr: {self.stderr}")
            raise e

    def get_action_to_cache_miss_count(self) -> Dict[str, int]:
        """
        Returns a dictionary of action key to number of cache misses.
        Populates this dictionary by going through stdout looking for logs of compute calls.

        Currently, there is no unique identifier for the action in yak, so the action key
        is just a tuple of configured target name and analysis id.
        """
        action_to_cache_miss_count: Dict[str, int] = defaultdict(int)
        for line in self.stdout.splitlines():
            if LOG_COMPUTE_KEY in line:
                target = line.split(LOG_COMPUTE_KEY)[-1].strip()
                action_to_cache_miss_count[target] += 1
        return dict(action_to_cache_miss_count)


class AuditConfigResult(YakResult):
    """Represents a yak process of an audit config command that has finished running"""

    def __init__(self, base: YakResult) -> None:
        self.__dict__.update(base.__dict__)

    def get_json(self) -> Dict[str, str]:
        """Returns a dict of the json sent back by yak"""
        assert "--style=json" in self.yak_args or "--style json" in self.yak_args, (
            "Must add --style=json or `--style json` arg to get json output"
        )
        try:
            start = self.stdout.index("{")
            audit_json = self.stdout[start:].strip()
            parsed = json.loads(audit_json)
            return parsed
        except Exception as e:
            print(f"stdout: {self.stdout}\nstderr: {self.stderr}")
            raise e
