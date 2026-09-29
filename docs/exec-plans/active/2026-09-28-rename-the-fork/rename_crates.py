#!/usr/bin/env python3
"""Milestone 5 of the rename: the Cargo packages named `buck2*` and their
directories take `yak` names, so `buck2_core` becomes `yak_core` and the
package `buck2` becomes `yak`.

Run from the repository root of a clean checkout of commit b2bb73b419:

    python3 path/to/rename_crates.py            # prints the changes it would make
    python3 path/to/rename_crates.py --apply    # moves the directories and edits the files

After --apply, run manual_edits_crates.py, then `cargo update --workspace
--offline` and `cargo fmt --all`.
"""

import argparse
import collections
import json
import os
import re
import subprocess
import sys

# The workspace packages that take the new name. The script checks the list
# against `cargo metadata`.
CRATES = """
buck2 buck2_action_impl buck2_action_impl_tests buck2_action_metadata_proto
buck2_action_parallelism buck2_analysis buck2_anon_target buck2_artifact
buck2_build_api buck2_build_api_derive buck2_build_api_tests buck2_build_info
buck2_build_signals buck2_build_signals_impl buck2_bxl buck2_certs
buck2_cfg_constructor buck2_cli_proto buck2_client buck2_client_ctx
buck2_cmd_audit_client buck2_cmd_audit_server buck2_cmd_completion_client
buck2_cmd_debug_client buck2_cmd_docs_client buck2_cmd_docs_server
buck2_cmd_log_client buck2_cmd_query_server buck2_cmd_starlark_client
buck2_cmd_starlark_server buck2_cmd_targets_server buck2_common
buck2_concurrency buck2_configured buck2_core buck2_critical_path buck2_daemon
buck2_data buck2_directory buck2_downward_api buck2_downward_api_proto
buck2_embedded_section buck2_env buck2_error buck2_error_derive
buck2_error_tests buck2_errorformat buck2_event_log buck2_event_observer
buck2_events buck2_execute buck2_execute_impl buck2_execute_local
buck2_external_cells buck2_external_cells_bundled buck2_file_watcher
buck2_forkserver buck2_forkserver_proto buck2_fs buck2_grpc buck2_hash
buck2_host_sharing_proto buck2_http buck2_install_proto buck2_interpreter
buck2_interpreter_for_build buck2_interpreter_for_build_tests buck2_log_common
buck2_miniperf buck2_miniperf_proto buck2_node buck2_node_tests
buck2_offline_archive buck2_profile buck2_proto_serde buck2_protoc_dev
buck2_query buck2_query_derive buck2_query_impls buck2_query_parser
buck2_re_configuration buck2_resource_control buck2_server
buck2_server_commands buck2_server_ctx buck2_server_starlark_debug
buck2_sketches buck2_subscription_proto buck2_test buck2_test_api
buck2_test_proto buck2_test_runner buck2_transition buck2_util
buck2_validation buck2_worker_proto buck2_wrapper_common
""".split()

# Paths whose text keeps the old names.
SKIPPED = (
    # The plans record what happened under the old names, and the scripts beside
    # them hold the old names as patterns.
    "docs/exec-plans/active/",
    "docs/exec-plans/completed/",
    # The history lists the old names. A hand edit adds the entry for this change.
    "CHANGELOG.md",
    # `cargo update --workspace` rewrites the lock file in sorted order.
    "Cargo.lock",
    # `buck2_query` in the Erlang shell of the prelude names an Erlang function.
    "prelude/erlang/",
)

# Files that are named after a crate, apart from the crate directories.
FILE_MOVES = {
    "app/yak/bin/buck2.rs": "app/yak/bin/yak.rs",
    "app/yak_build_api/src/build/detailed_aggregated_metrics/buck2_sketches.rs": (
        "app/yak_build_api/src/build/detailed_aggregated_metrics/yak_sketches.rs"
    ),
}

# Identifiers that start with a crate name and name the same thing as the
# crate, or a sibling of a renamed macro, so they take the new prefix too.
SIBLINGS = {
    # The macros `buck2_env!` and `buck2_env_name!` of the crate `buck2_env`.
    "buck2_env_name": "yak_env_name",
    # The function that the `buck2_error!` macro calls.
    "buck2_error_impl": "yak_error_impl",
    # The constraint and the transition macro that build the client-only binary
    # of the crate `buck2`.
    "buck2_client_only_setting": "yak_client_only_setting",
    "buck2_client_transition_alias": "yak_client_transition_alias",
    # The test target of the crate `buck2_miniperf`.
    "buck2_miniperf_test": "yak_miniperf_test",
}


def new_name(crate):
    return "yak" + crate[len("buck2") :]


def word(names):
    """Matches any of the names where no letter, digit, or underscore touches it."""
    alternatives = "|".join(sorted(names, key=len, reverse=True))
    return re.compile(r"(?<![A-Za-z0-9_])(" + alternatives + r")(?![A-Za-z0-9_])")


RULES = [
    # A crate name, or an identifier that shares it, such as the `buck2_error!`
    # macro, the module path `buck2_core::fs`, or the label
    # `//app/buck2_core:buck2_core`.
    ("crate", word(c for c in CRATES if c != "buck2"), lambda m: new_name(m.group(1))),
    ("sibling", word(SIBLINGS), lambda m: SIBLINGS[m.group(1)]),
    # The directory of the crate `buck2`, in paths and labels.
    ("app/buck2", re.compile(r"(?<![A-Za-z0-9_])app/buck2(?![A-Za-z0-9_-])"), lambda m: "app/yak"),
    # The targets `buck2` and `buck2-bin` in labels of that directory, such as
    # `//app/buck2:buck2-bin`. The rule above has renamed the directory.
    ("app/yak:buck2", re.compile(r"(?<=app/yak:)buck2(?![A-Za-z0-9_])"), lambda m: "yak"),
    # The binary target of the crate `buck2` and its source file.
    ("buck2-bin", re.compile(r"(?<![A-Za-z0-9_-])buck2-bin(?![A-Za-z0-9_-])"), lambda m: "yak-bin"),
    ("bin/buck2.rs", re.compile(r"(?<![A-Za-z0-9_])bin/buck2\.rs"), lambda m: "bin/yak.rs"),
    # The attribute of the error derive macro, `#[buck2(tag = Input)]`.
    ("#[buck2(", re.compile(r"#\[buck2\("), lambda m: "#[yak("),
]

# URLs of the upstream project keep its paths.
URL = re.compile(r"https?://[^\s)\]>\"'`]+")
UPSTREAM = ("github.com/facebook/", "github.com/facebookincubator/")


def git(*args, capture=False):
    result = subprocess.run(
        ["git", "-c", "submodule.recurse=false", *args],
        check=True,
        capture_output=capture,
        text=True,
    )
    return result.stdout


def check_crates():
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    root = os.getcwd()
    found = {}
    for package in metadata["packages"]:
        if package["name"].startswith("buck2"):
            found[package["name"]] = os.path.relpath(os.path.dirname(package["manifest_path"]), root)
    if sorted(found) != sorted(CRATES):
        sys.exit(f"cargo metadata lists other buck2 packages: {sorted(set(found) ^ set(CRATES))}")
    for crate, directory in found.items():
        if os.path.basename(directory) != crate:
            sys.exit(f"{crate} lives in {directory}, which has another name")
    return found


def edit_text(text):
    """Returns the edited text and a count of the replacements per rule."""
    counts = collections.Counter()
    pieces = []
    last = 0
    for url in URL.finditer(text):
        if any(host in url.group(0) for host in UPSTREAM):
            pieces.append((False, text[last : url.start()]))
            pieces.append((True, url.group(0)))
            last = url.end()
    pieces.append((False, text[last:]))
    out = []
    for protected, piece in pieces:
        if not protected:
            for name, pattern, replacement in RULES:
                piece, n = pattern.subn(replacement, piece)
                counts[name] += n
        out.append(piece)
    return "".join(out), counts


def toml_key(line):
    return re.match(r"\s*([A-Za-z0-9_-]+)", line).group(1)


def item_key(line):
    return line.strip().rstrip(",")


# A dependency on one line, such as `buck2_core.workspace = true` or
# `buck2_core = { path = "app/buck2_core" }`.
TOML_ENTRY = re.compile(r"^[A-Za-z0-9_-]+(\.workspace)?\s*=\s*[^\[{]*(\{[^}]*\})?[^\[{]*$")
# A string on its own line in a list, such as a member of the workspace.
TOML_ITEM = re.compile(r'^\s*"[^"]*",?\s*$')
# A label on its own line in a list of a build file, such as a dependency.
LABEL_ITEM = re.compile(r'^\s*"([A-Za-z0-9_-]*//|:)[^"]*",?\s*$')


def resort(old_lines, new_lines, pattern, key, commas):
    """Sorts each run of items that the rules changed and that was sorted before."""
    out = list(new_lines)
    i = 0
    while i < len(new_lines):
        if not (pattern.match(old_lines[i]) and pattern.match(new_lines[i])):
            i += 1
            continue
        j = i
        while j < len(new_lines) and pattern.match(old_lines[j]) and pattern.match(new_lines[j]):
            j += 1
        before, after = old_lines[i:j], new_lines[i:j]
        if before != after and before == sorted(before, key=key):
            if commas:
                # Every item keeps a comma, except the last one when it had none.
                last_comma = after[-1].rstrip().endswith(",")
                items = sorted((line.rstrip().rstrip(",") for line in after), key=key)
                after = [item + ",\n" for item in items]
                if not last_comma:
                    after[-1] = items[-1] + "\n"
                out[i:j] = after
            else:
                out[i:j] = sorted(after, key=key)
        i = j
    return out


def resort_file(path, old_text, new_text):
    old_lines = old_text.splitlines(keepends=True)
    new_lines = new_text.splitlines(keepends=True)
    if len(old_lines) != len(new_lines):
        return new_text
    name = os.path.basename(path)
    if name == "Cargo.toml":
        lines = resort(old_lines, new_lines, TOML_ENTRY, toml_key, commas=False)
        lines = resort(old_lines, lines, TOML_ITEM, item_key, commas=True)
    elif name == "YAK" or name.endswith((".bzl", ".bxl")):
        lines = resort(old_lines, new_lines, LABEL_ITEM, item_key, commas=True)
    else:
        return new_text
    return "".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    directories = check_crates()

    # `git grep -I` leaves out binary files, such as the event logs of the tests.
    files = git("grep", "-l", "-z", "-I", "-e", "buck2", "--", ".", capture=True).split("\0")
    totals = collections.Counter()
    changed = {}
    for path in files:
        if not path or path.startswith(SKIPPED) or os.path.islink(path):
            continue
        with open(path, encoding="utf-8") as f:
            text = f.read()
        new_text, counts = edit_text(text)
        if new_text != text:
            changed[path] = resort_file(path, text, new_text)
            totals.update(counts)

    moves = sorted(
        (directory, os.path.join(os.path.dirname(directory), new_name(crate)))
        for crate, directory in directories.items()
    )
    print(f"{len(moves)} directories move, {len(FILE_MOVES)} files move")
    print(f"{sum(totals.values())} replacements in {len(changed)} files: {dict(totals)}")
    if not args.apply:
        for path in sorted(changed):
            print(path)
        return

    for path, text in changed.items():
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
    for old, new in moves:
        git("mv", old, new)
    for old, new in FILE_MOVES.items():
        git("mv", old, new)


if __name__ == "__main__":
    main()
