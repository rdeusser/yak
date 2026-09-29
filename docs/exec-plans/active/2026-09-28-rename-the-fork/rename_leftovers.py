#!/usr/bin/env python3
"""Runtime names and prose that milestones 1 and 6 of the rename missed.

Run from the repository root of a clean checkout of commit 5bf8000575:

    python3 path/to/rename_leftovers.py

Each edit names its file, the exact text it replaces, and how many times that
text occurs, so a rerun on a changed tree fails instead of editing blindly.
"""

import subprocess
import sys

failures = []


def edit(path, old, new, count=1):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    n = text.count(old)
    if n != count:
        failures.append(f"{path}: expected {count} of {old!r}, found {n}")
        return
    with open(path, "w", encoding="utf-8") as f:
        f.write(text.replace(old, new))


# Thread names, which thread dumps and panic messages print.
THREADS = [
    ("app/buck2/src/lib.rs", "buck2-main", "yak-main"),
    ("app/buck2_client_ctx/src/tokio_runtime_setup.rs", "buck2-cli", "yak-cli"),
    ("app/buck2_daemon/src/daemon.rs", "buck2-rt", "yak-rt"),
    ("app/buck2_daemon/src/daemon.rs", "buck2-tn", "yak-tn"),
    ("app/buck2_daemon/src/no_buckd.rs", "buck2-no-buckd", "yak-no-yakd"),
    ("app/buck2_daemon/src/schedule_termination.rs", "buck2-terminate-after", "yak-terminate-after"),
    ("app/buck2_execute/src/execute/blocking.rs", "buck-io-{i}", "yak-io-{i}"),
    ("app/buck2_execute_impl/src/materializers/deferred.rs", "buck2-dm", "yak-dm"),
    ("app/buck2_execute_impl/src/materializers/deferred/tests.rs", "buck2-dm", "yak-dm"),
    ("app/buck2_execute_impl/src/sqlite/dep_file_state_db.rs", "buck2-dep-file-db", "yak-dep-file-db"),
    ("app/buck2_server/src/daemon/crash.rs", "buck2-crash", "yak-crash"),
    ("app/buck2_server/src/daemon/server.rs", "buck2-shutdown-watchdog", "yak-shutdown-watchdog"),
    ("app/buck2_server/src/dice_tracker.rs", "buck2-dice-tracker", "yak-dice-tracker"),
    ("dice/dice/src/core/processor.rs", "buck2-dice", "yak-dice"),
]
for path, old, new in THREADS:
    edit(path, f'"{old}"', f'"{new}"')
edit(
    "app/buck2_client_ctx/src/subscribers/classify_server_stderr.rs",
    "        // thread 'buck2-dm' has overflowed its stack\n",
    "        // thread 'yak-dm' has overflowed its stack\n",
)
edit(
    "app/buck2_server/src/snapshot.rs",
    "    /// Handle to the *main* (`buck2-rt`) runtime where DICE / build work runs.\n"
    "    /// Snapshot collection itself runs on the smaller `buck2-tn` Tonic\n",
    "    /// Handle to the *main* (`yak-rt`) runtime where DICE / build work runs.\n"
    "    /// Snapshot collection itself runs on the smaller `yak-tn` Tonic\n",
)

# The process name of the forkserver, which `ps` prints.
edit("app/buck2_forkserver/src/launch.rs", '.arg0("(buck2-forkserver)")', '.arg0("(yak-forkserver)")')

# The temporary file that `yak build --out` writes before it renames it.
edit("app/buck2_client/src/commands/build/out.rs", 'tmp_name.push(".buck2.tmp");', 'tmp_name.push(".yak.tmp");')

# The directory of worker sockets under /tmp, and the directory of saved games
# in the home directory.
edit("app/buck2_execute_impl/src/executors/worker.rs", '"/tmp/buck2_worker"', '"/tmp/yak_worker"')
edit(
    "app/buck2_execute_impl/src/executors/worker.rs",
    '"/tmp/buck2_worker_test/{name}"',
    '"/tmp/yak_worker_test/{name}"',
)
edit("app/buck2_client_ctx/src/subscribers/superconsole.rs", '.join(".buck2_games")', '.join(".yak_games")')
edit("games/bin/menu.rs", '.join(".buck2_games")', '.join(".yak_games")')

# Remote Execution use cases: the default that yak sends when no use case is
# configured, and the values in the examples and tests.
USE_CASES = {
    "buck2-default": "yak-default",
    "buck2-local-unmaterialization": "yak-local-unmaterialization",
    "buck2-testing": "yak-testing",
    "buck2-user": "yak-user",
}
paths = subprocess.run(
    ["git", "-c", "submodule.recurse=false", "grep", "-l", "-I", "-E", "|".join(USE_CASES), "--", ".", ":!docs/exec-plans/"],
    check=True,
    capture_output=True,
    text=True,
).stdout.split()
for path in paths:
    with open(path, encoding="utf-8") as f:
        text = f.read()
    for old, new in USE_CASES.items():
        text = text.replace(old, new)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)

# Prose that the rules of rename_code_prose.py skipped, because the name stood
# before a colon or a hyphen, which also follow the name in labels and paths.
edit(
    "app/buck2_client/src/commands/run.rs",
    "/// Use `--` to separate arguments to the target from arguments to buck2:\n",
    "/// Use `--` to separate arguments to the target from arguments to yak:\n",
)
edit(
    "app/buck2_client_ctx/src/daemon/client/kill.rs",
    '"unexpected error connecting to Buck2: {:#} \\',
    '"unexpected error connecting to yak: {:#} \\',
)
edit(
    "app/buck2_bxl/src/bxl/starlark_defs/context.rs",
    "            //   which is inconsistent with the rest of buck2:\n",
    "            //   which is inconsistent with the rest of yak:\n",
)
edit(
    "app/buck2_build_api/src/interpreter/rule_defs/type_id_domain.rs",
    "//! than starlark-rust, which is buck2-agnostic and only knows its own\n",
    "//! than starlark-rust, which is yak-agnostic and only knows its own\n",
)
edit(
    "app/buck2_build_api/src/interpreter/rule_defs/type_id_domain.rs",
    "/// [`TypeIdDomain`]s for buck2-defined nominal types.\n",
    "/// [`TypeIdDomain`]s for yak-defined nominal types.\n",
)
edit(
    "app/buck2_interpreter_for_build/src/interpreter/build_context.rs",
    "/// Buck-specific information exposed to the starlark environment via the context's extra field.\n",
    "/// yak-specific information exposed to the starlark environment via the context's extra field.\n",
)
edit(
    "app/buck2_interpreter_for_build/src/interpreter/global_interpreter_state.rs",
    "    /// (primarily starlark stdlib and Buck-provided functions).\n",
    "    /// (primarily starlark stdlib and yak-provided functions).\n",
)
edit(
    "integrations/rust-project/src/project_json.rs",
    "//! buck-based projects. For additional details, see rust-analyzer's [documentation].\n",
    "//! yak-based projects. For additional details, see rust-analyzer's [documentation].\n",
)
edit(
    "dice/fuzzy_dice/src/execution.rs",
    "                    // store. This is the buck2-file-watcher pattern: dice's\n",
    "                    // store. This is the pattern of yak's file watcher: dice's\n",
)
# `[repositories]` was Buck1's name for `[cells]`.
edit(
    "app/buck2_common/src/legacy_configs/cells.rs",
    "        // that we'll ever remove `repositories` since that would break existing projects.\n"
    "        //\n"
    "        // Note that `cells` is buck2-only\n",
    "        // that we'll ever remove `repositories` since that would break existing projects.\n",
)

if failures:
    print("\n".join(failures))
    sys.exit(1)
print(f"leftover edits applied, use cases renamed in {len(paths)} files")
