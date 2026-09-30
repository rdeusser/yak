#!/usr/bin/env python3
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Ports commits of facebook/buck2 to yak, one yak commit per upstream commit.

Each file an upstream commit changes is renamed to its yak path, and its
contents before and after the commit take the yak names. `git merge-file`
applies the difference between the two to the yak file. Build files and
Cargo.lock files take their own strategies, described at `port_build_file`
and `update_lock_files`.

Run `port.py --help` for the commands.
"""

import argparse
import dataclasses
import difflib
import enum
import json
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from pathlib import Path

FORK_POINT = "903bfd7a61"
UPSTREAM = "upstream/main"
UPSTREAM_URL = "https://github.com/facebook/buck2.git"
TRAILER = "Ported from facebook/buck2@"

TOOL_DIR = Path(__file__).resolve().parent
SKIPPED = TOOL_DIR / "skipped.txt"
# The ledger of skipped commits changes between ports, and its changes are
# committed on their own.
NOT_THE_LEDGER = f":!{SKIPPED.relative_to(TOOL_DIR.parent.parent)}"

# Text that names the upstream project keeps its upstream names.
PROTECTED = re.compile(
    r"github\.com/facebook(?:incubator)?/buck2?(?:-change-detector)?\b"
    r"|buck2?\.build\b"
    r"|Buck1\b"
    r"|facebook/buck2?\b"
)
UPSTREAM_NAME = re.compile(r"(?i)buck2?(?!et)")
OSS_DISABLE = re.compile(r"^\s*(?:#|//)\s*@oss-disable(?:\[end= \])?:")
OSS_ENABLE = re.compile(r"\s*(?:#|//)\s*@oss-enable\s*$")
# Labels of Meta's repository that name this repository's code or its vendored crates.
# Files that the fork moved and changed too much for Git's rename detection to
# pair them, keyed by the yak form of the upstream path.
MOVES = {
    "prelude/toolchains/demo.bzl": "prelude/toolchains/system.bzl",
}
LABELS = [
    (re.compile(r"fbsource//third-party/rust:"), "//third-party/rust:"),
    (re.compile(r"(?:fbcode)?//buck2/"), "//"),
]


def yak_name(match: re.Match) -> str:
    """The yak form of an upstream name, in the case of the original."""
    name = match.group(0)
    if name.isupper():
        return "YAK"
    if name[0].isupper():
        return "Yak"
    return "yak"


def rename(text: str) -> str:
    """Replaces the upstream names in `text` with yak names, apart from text that
    names the upstream project."""
    for pattern, replacement in LABELS:
        text = pattern.sub(replacement, text)
    out = []
    last = 0
    for protected in PROTECTED.finditer(text):
        out.append(UPSTREAM_NAME.sub(yak_name, text[last : protected.start()]))
        out.append(protected.group(0))
        last = protected.end()
    out.append(UPSTREAM_NAME.sub(yak_name, text[last:]))
    return "".join(out)


def open_source_side(text: str) -> str:
    """Drops the lines that upstream disables in its open-source export, and
    unwraps the lines that the export enables."""
    lines = []
    for line in text.splitlines(keepends=True):
        if OSS_DISABLE.match(line):
            continue
        body = line.rstrip("\r\n")
        ending = line[len(body) :]
        stripped = OSS_ENABLE.sub("", body)
        lines.append((stripped if stripped != body else body) + ending)
    return "".join(lines)


def transform(text: str) -> str:
    """The yak form of the contents of an upstream file."""
    return rename(open_source_side(text))


def transform_path(path: str) -> str:
    return rename(path)


class Git:
    def __init__(self, root: Path) -> None:
        self.root = root

    def run(
        self,
        *args: str,
        input: bytes | None = None,
        check: bool = True,
        env: dict | None = None,
    ) -> bytes:
        result = subprocess.run(
            ["git", *args],
            cwd=self.root,
            input=input,
            capture_output=True,
            check=False,
            env=env,
        )
        if check and result.returncode != 0:
            raise SystemExit(
                f"git {' '.join(args)} failed:\n{result.stderr.decode(errors='replace')}"
            )
        return result.stdout

    def text(self, *args: str) -> str:
        return self.run(*args).decode()

    def blob(self, rev: str, path: str) -> bytes | None:
        result = subprocess.run(
            ["git", "cat-file", "blob", f"{rev}:{path}"],
            cwd=self.root,
            capture_output=True,
            check=False,
        )
        return result.stdout if result.returncode == 0 else None

    def files(self, rev: str) -> set[str]:
        return {
            p
            for p in self.run("ls-tree", "-r", "-z", "--name-only", rev)
            .decode()
            .split("\0")
            if p
        }


@dataclasses.dataclass
class PathMap:
    """PathMap maps upstream paths to paths of this repository.

    `moved` holds the files that the fork moved, found by Git's rename detection
    between the fork point with yak names and `HEAD`. `removed` holds the files
    of the fork point that the fork deleted. `dirs` maps the directories of moved
    files, for files that upstream adds after the fork point.
    """

    head_files: set[str]
    moved: dict[str, str]
    removed: set[str]
    dirs: dict[str, str]
    head_dirs: set[str]
    fork_dirs: set[str]

    def target(self, upstream_path: str) -> str | None:
        """The path of this repository that holds `upstream_path`, or None when
        the fork removed the file or its directory."""
        path = transform_path(upstream_path)
        if path in MOVES:
            return MOVES[path]
        if path in self.moved:
            return self.moved[path]
        if path in self.removed:
            return None
        if path in self.head_files:
            return path
        # A file that upstream added after the fork point goes where the fork put
        # its directory. A directory that the fork point had and `HEAD` lacks
        # was removed.
        parts = path.split("/")
        for i in range(len(parts) - 1, 0, -1):
            dir = "/".join(parts[:i])
            if dir in self.dirs:
                return self.dirs[dir] + "/" + "/".join(parts[i:])
            if dir in self.head_dirs:
                return path
            if dir in self.fork_dirs:
                return None
        return path


def dirs_of(files: set[str]) -> set[str]:
    dirs = set()
    for f in files:
        parts = f.split("/")
        for i in range(1, len(parts)):
            dirs.add("/".join(parts[:i]))
    return dirs


def build_path_map(git: Git) -> PathMap:
    """Writes the fork point with yak names into a tree, and compares it with
    `HEAD`. The result is cached in `.git/yak-port/` by the tree of `HEAD` and
    the source of this tool."""
    cache_dir = Path(git.text("rev-parse", "--git-path", "yak-port").strip())
    if not cache_dir.is_absolute():
        cache_dir = git.root / cache_dir
    cache_dir.mkdir(parents=True, exist_ok=True)
    head_tree = git.text("rev-parse", "HEAD^{tree}").strip()
    tool = git.run("hash-object", "--", str(Path(__file__).resolve())).decode().strip()
    cache = cache_dir / f"pathmap-{head_tree}-{tool}.json"
    if cache.exists():
        data = json.loads(cache.read_text())
    else:
        synthetic = renamed_fork_tree(git, cache_dir, tool)
        raw = (
            git.run(
                "-c",
                "diff.renameLimit=0",
                "diff",
                "--name-status",
                "-z",
                "-M50%",
                "--no-ext-diff",
                synthetic,
                head_tree,
            )
            .decode()
            .split("\0")
        )
        moved, removed = {}, []
        i = 0
        while i < len(raw) and raw[i]:
            status = raw[i]
            if status.startswith("R"):
                moved[raw[i + 1]] = raw[i + 2]
                i += 3
            else:
                if status == "D":
                    removed.append(raw[i + 1])
                i += 2
        data = {
            "moved": moved,
            "removed": removed,
            "fork": sorted(git.files(synthetic)),
        }
        cache.write_text(json.dumps(data))
    head_files = git.files("HEAD")
    moved = data["moved"]
    dirs = {}
    for src, dst in moved.items():
        # Strip the path components that the two paths share at their ends, so
        # that `docs/a/b.md` -> `website/docs/a/b.md` maps `docs` -> `website/docs`.
        s, d = src.split("/"), dst.split("/")
        while len(s) > 1 and len(d) > 1 and s[-1] == d[-1]:
            s.pop()
            d.pop()
        if len(s) < len(src.split("/")):
            dirs.setdefault("/".join(s), "/".join(d))
    return PathMap(
        head_files=head_files,
        moved=moved,
        removed=set(data["removed"]),
        dirs=dirs,
        head_dirs=dirs_of(head_files),
        fork_dirs=dirs_of(set(data["fork"])),
    )


def renamed_fork_tree(git: Git, cache_dir: Path, tool: str) -> str:
    """The ID of a tree that holds the fork point's files with yak paths and contents."""
    cached = cache_dir / f"forktree-{tool}"
    if cached.exists():
        return cached.read_text().strip()
    listing = [
        e for e in git.run("ls-tree", "-r", "-z", FORK_POINT).decode().split("\0") if e
    ]
    blobs = []
    for entry in listing:
        meta, path = entry.split("\t", 1)
        mode, kind, obj = meta.split(" ")
        if kind == "blob":
            blobs.append((mode, obj, path))
    contents = git.run(
        "cat-file", "--batch", input="".join(f"{obj}\n" for _, obj, _ in blobs).encode()
    )
    with tempfile.TemporaryDirectory() as tmp:
        files = []
        offset = 0
        for n, (mode, obj, path) in enumerate(blobs):
            header_end = contents.index(b"\n", offset)
            size = int(contents[offset:header_end].split(b" ")[2])
            data = contents[header_end + 1 : header_end + 1 + size]
            offset = header_end + 1 + size + 1
            text = decode(data)
            f = Path(tmp) / str(n)
            f.write_bytes(transform(text).encode() if text is not None else data)
            files.append(str(f))
        objects = (
            git.run(
                "hash-object",
                "-w",
                "--no-filters",
                "--stdin-paths",
                input="\n".join(files).encode(),
            )
            .decode()
            .split()
        )
        entries = [
            f"{mode} blob {new}\t{transform_path(path)}"
            for (mode, _, path), new in zip(blobs, objects)
        ]
        index = Path(tmp) / "index"
        env = dict(os.environ, GIT_INDEX_FILE=str(index))
        git.run(
            "update-index",
            "--add",
            "--index-info",
            input="\n".join(entries).encode() + b"\n",
            env=env,
        )
        tree = git.run("write-tree", env=env).decode().strip()
    cached.write_text(tree)
    return tree


@dataclasses.dataclass
class Change:
    status: str
    old: str | None
    new: str | None


def upstream_changes(git: Git, commit: str) -> list[Change]:
    raw = (
        git.run(
            "diff-tree",
            "-r",
            "-z",
            "-M",
            "--no-commit-id",
            "--name-status",
            f"{commit}^",
            commit,
        )
        .decode()
        .split("\0")
    )
    changes = []
    i = 0
    while i < len(raw) and raw[i]:
        status = raw[i][0]
        if status in "RC":
            changes.append(Change(status, raw[i + 1], raw[i + 2]))
            i += 3
        else:
            path = raw[i + 1]
            changes.append(
                Change(
                    status,
                    None if status == "A" else path,
                    None if status == "D" else path,
                )
            )
            i += 2
    return changes


@dataclasses.dataclass
class Outcome:
    written: list[str] = dataclasses.field(default_factory=list)
    conflicts: list[str] = dataclasses.field(default_factory=list)
    review: list[str] = dataclasses.field(default_factory=list)
    dropped: list[str] = dataclasses.field(default_factory=list)
    lock_files: list[str] = dataclasses.field(default_factory=list)
    notes: list[str] = dataclasses.field(default_factory=list)


def decode(data: bytes | None) -> str | None:
    if data is None:
        return None
    try:
        return data.decode()
    except UnicodeDecodeError:
        return None


def merge(ours: str, base: str, theirs: str) -> tuple[str, bool, list[str]]:
    """Three-way merges text with `git merge-file`. Returns the result, whether
    it has conflicts, and notes on the conflicts that were resolved."""
    with tempfile.TemporaryDirectory() as tmp:
        paths = []
        for name, text in (("yak", ours), ("before", base), ("upstream", theirs)):
            p = Path(tmp) / name
            p.write_text(text)
            paths.append(str(p))
        result = subprocess.run(
            [
                "git",
                "merge-file",
                "-p",
                "--diff3",
                "-L",
                "yak",
                "-L",
                "upstream-before",
                "-L",
                "upstream",
                *paths,
            ],
            capture_output=True,
            check=False,
        )
        if result.returncode < 0 or result.returncode > 127:
            raise SystemExit(f"git merge-file failed: {result.stderr.decode()}")
        merged = result.stdout.decode()
        if result.returncode == 0:
            return merged, False, []
        return resolve_conflicts(merged)


CONFLICT = re.compile(
    r"^<<<<<<< yak\n(.*?)^\|\|\|\|\|\|\| upstream-before\n(.*?)^=======\n(.*?)^>>>>>>> upstream\n",
    re.MULTILINE | re.DOTALL,
)


def resolve_conflicts(merged: str) -> tuple[str, bool, list[str]]:
    """Resolves the conflict blocks of `git merge-file --diff3` output that
    have a safe resolution:

    - The two sides edit separate lines of the merge base. `git merge-file`
      reports a conflict when the edits are adjacent, such as when the fork
      removed the line after a line that upstream changed.
    - The fork deleted the lines that upstream changed, as it deleted Meta's
      internal code.

    Returns the result, whether conflicts remain, and notes on the blocks
    resolved by keeping the fork's deletion."""
    remaining = False
    notes = []

    def resolve(block: re.Match) -> str:
        nonlocal remaining
        ours, base, theirs = (
            block.group(i).splitlines(keepends=True) for i in (1, 2, 3)
        )
        combined = combine_edits(base, edits(base, ours), edits(base, theirs))
        if combined is not None:
            return "".join(combined)
        if not ours:
            first = next((line.strip() for line in base if line.strip()), "")
            notes.append(
                f"kept the fork's deletion of {len(base)} lines that upstream changed, from `{first}`"
            )
            return ""
        remaining = True
        return block.group(0)

    return CONFLICT.sub(resolve, merged), remaining, notes


def rustfmt(git: Git, text: str | None) -> str | None:
    """Formats Rust source as the fork formats it. The yak names are shorter
    than the upstream names, so `rustfmt` wraps the fork's lines differently
    from upstream's. Returns `text` unchanged when `rustfmt` rejects it."""
    if text is None:
        return None
    result = subprocess.run(
        [
            "rustfmt",
            "--edition",
            "2024",
            "--config-path",
            str(git.root / "rustfmt.toml"),
        ],
        cwd=git.root,
        input=text,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout if result.returncode == 0 else text


def adopt_fork_spelling(ours: str, base: str, theirs: str) -> tuple[str, str]:
    """Rewrites the lines of `base` and `theirs` that differ from a line of the
    fork's file only in the case of `yak` to the fork's line. The fork writes
    the tool's name in lowercase in some prose where the renamed upstream text
    has `Yak`, and those lines would otherwise conflict."""
    fork = {}
    for line in ours.splitlines(keepends=True):
        fork.setdefault(line.replace("Yak", "yak"), line)
    ours_lines = set(fork.values())

    def adopt(text: str) -> str:
        lines = []
        for line in text.splitlines(keepends=True):
            if line not in ours_lines and "Yak" in line:
                line = fork.get(line.replace("Yak", "yak"), line)
            lines.append(line)
        return "".join(lines)

    return adopt(base), adopt(theirs)


USE_LINE = re.compile(r"^(?:pub(?:\([^)]*\))? )?use [^{}\n]*;\n", re.MULTILINE)


def sort_use_runs(text: str) -> str:
    """Sorts each run of one-line `use` items. The yak names sort differently
    from the upstream names, so the fork's imports are in another order than
    upstream's. Sorting all three sides before a merge keeps the order out of
    the merge, and `rustfmt` sorts the result as the fork does."""
    lines = text.splitlines(keepends=True)
    result = []
    run: list[str] = []
    for line in lines:
        if USE_LINE.fullmatch(line):
            run.append(line)
            continue
        result.extend(sorted(run))
        run = []
        result.append(line)
    result.extend(sorted(run))
    return "".join(result)


def edits(base: list[str], other: list[str]) -> list[tuple[int, int, list[str]]]:
    """The edits that turn `base` into `other`, each as the range of base lines
    it replaces and the lines that replace them."""
    matcher = difflib.SequenceMatcher(None, base, other, autojunk=False)
    return [
        (i1, i2, other[j1:j2])
        for op, i1, i2, j1, j2 in matcher.get_opcodes()
        if op != "equal"
    ]


class Applied(enum.Enum):
    NONE = enum.auto()
    SOME = enum.auto()
    ALL = enum.auto()


def already_applied(ours: str, base: str, theirs: str) -> Applied:
    """Whether the fork's file already has upstream's edits, such as a golden
    file that the fork regenerated. A merge would apply such an edit a second
    time. An edit counts as applied when the fork's file holds the lines it
    adds, upstream's file before the edit lacked them, and the fork's file
    lacks the lines it replaces. Lines that are too short to identify a place,
    such as `}`, never count."""
    ours_lines = ours.splitlines(keepends=True)
    upstream = edits(base.splitlines(keepends=True), theirs.splitlines(keepends=True))
    base_lines = base.splitlines(keepends=True)
    applied = 0
    for start, end, lines in upstream:
        replaced = base_lines[start:end]
        if (
            any(len(line.strip()) >= 8 for line in lines)
            and contains(ours_lines, lines)
            and not contains(base_lines, lines)
            and not (replaced and contains(ours_lines, replaced))
        ):
            applied += 1
    if applied == 0:
        return Applied.NONE
    return Applied.ALL if applied == len(upstream) else Applied.SOME


def contains(lines: list[str], block: list[str]) -> bool:
    n = len(block)
    return any(lines[i : i + n] == block for i in range(len(lines) - n + 1))


def combine_edits(
    base: list[str],
    ours: list[tuple[int, int, list[str]]],
    theirs: list[tuple[int, int, list[str]]],
) -> list[str] | None:
    """Applies both sides' edits to `base`, or returns None when an edit of one
    side overlaps an edit of the other. An edit that both sides made counts
    once."""
    theirs = [e for e in theirs if e not in ours]
    for a1, a2, _ in ours:
        for b1, b2, _ in theirs:
            if max(a1, b1) < min(a2, b2) or (a1 == a2 == b1 == b2):
                return None
            # An insertion inside the other side's range overlaps it.
            if (a1 == a2 and b1 < a1 < b2) or (b1 == b2 and a1 < b1 < a2):
                return None
    result = []
    position = 0
    for start, end, lines in sorted(ours + theirs, key=lambda e: (e[0], e[1])):
        result.extend(base[position:start])
        result.extend(lines)
        position = max(position, end)
    result.extend(base[position:])
    return result


BUILD_FILES = {"BUCK", "BUCK.v2", "TARGETS", "TARGETS.v2"}
LABEL_LINE = re.compile(r'^(\s*)"([^"]*//[^"]*)",\s*$')


def build_file_labels(text: str) -> dict[tuple[str, str], set[str]]:
    """Maps each (rule name, attribute) of a build file to the labels of its list
    value, for lists that hold one label per line."""
    labels: dict[tuple[str, str], set[str]] = {}
    rule = ""
    attr = None
    for line in text.splitlines():
        name = re.match(r'^\s+name = "([^"]+)"', line)
        if name:
            rule = name.group(1)
        opens = re.match(r"^\s+(\w+) = \[\s*$", line)
        if opens:
            attr = opens.group(1)
            continue
        if attr and re.match(r"^\s+\],?\s*$", line):
            attr = None
            continue
        label = LABEL_LINE.match(line)
        if attr and label:
            labels.setdefault((rule, attr), set()).add(label.group(2))
    return labels


def without_labels(text: str) -> list[str]:
    return [
        line.strip()
        for line in text.splitlines()
        if not LABEL_LINE.match(line) and line.strip()
    ]


def port_build_file(ours: str, base: str, theirs: str) -> tuple[str, bool]:
    """Ports a change to a build file. Upstream build files load Meta's macros
    and carry settings for Meta's tools, so their lines rarely match the `YAK`
    file. The labels that the change adds to or removes from a list attribute
    of a rule are added to or removed from the same list of the `YAK` file,
    which keeps its lists sorted. Returns the result and whether the change
    also did something else, which needs review."""
    before, after = build_file_labels(base), build_file_labels(theirs)
    lines = ours.splitlines(keepends=True)
    for key in sorted(set(before) | set(after)):
        removed = before.get(key, set()) - after.get(key, set())
        added = after.get(key, set()) - before.get(key, set())
        if not removed and not added:
            continue
        lines = edit_list(lines, key, removed, added)
        if lines is None:
            return ours, True
    other = without_labels(base) != without_labels(theirs)
    return "".join(lines), other


def edit_list(
    lines: list[str], key: tuple[str, str], removed: set[str], added: set[str]
) -> list[str] | None:
    rule, attr = key
    current_rule = ""
    start = end = None
    for i, line in enumerate(lines):
        name = re.match(r'^\s+name = "([^"]+)"', line)
        if name:
            current_rule = name.group(1)
        if current_rule == rule and re.match(rf"^\s+{attr} = \[\s*$", line):
            start = i
        elif start is not None and end is None and re.match(r"^\s+\],?\s*$", line):
            end = i
            break
    if start is None or end is None:
        return None
    body = lines[start + 1 : end]
    labels = [LABEL_LINE.match(l) for l in body]
    if not all(labels):
        return None
    indent = (
        labels[0].group(1)
        if labels
        else re.match(r"^(\s*)", lines[start]).group(1) + "    "
    )
    kept = [m.group(2) for m in labels if m.group(2) not in removed]
    new = kept + sorted(added - set(kept))
    if kept == sorted(kept):
        new = sorted(new)
    return lines[: start + 1] + [f'{indent}"{label}",\n' for label in new] + lines[end:]


def port_files(git: Git, commit: str, paths: PathMap) -> Outcome:
    outcome = Outcome()
    for change in upstream_changes(git, commit):
        upstream_path = change.new or change.old
        name = Path(upstream_path).name
        if name == "Cargo.lock":
            target = paths.target(upstream_path)
            if target:
                outcome.lock_files.append(target)
            else:
                outcome.dropped.append(upstream_path)
            continue
        old_target = paths.target(change.old) if change.old else None
        new_target = paths.target(change.new) if change.new else None
        if change.old and old_target is None and change.status != "A":
            outcome.dropped.append(upstream_path)
            continue
        if change.new and new_target is None:
            outcome.dropped.append(upstream_path)
            continue
        base_bytes = git.blob(f"{commit}^", change.old) if change.old else b""
        theirs_bytes = git.blob(commit, change.new) if change.new else None
        ours_path = old_target or new_target
        ours_bytes = (
            (git.root / ours_path).read_bytes()
            if ours_path and (git.root / ours_path).exists()
            else None
        )
        base, theirs, ours = (
            decode(base_bytes),
            decode(theirs_bytes),
            decode(ours_bytes),
        )
        base = transform(base) if base is not None else None
        theirs = transform(theirs) if theirs is not None else None
        if name.endswith(".rs"):
            base, theirs = (rustfmt(git, s) for s in (base, theirs))
        if None not in (ours, base, theirs):
            base, theirs = adopt_fork_spelling(ours, base, theirs)
        if name.endswith(".rs") and None not in (ours, base, theirs):
            ours, base, theirs = (sort_use_runs(s) for s in (ours, base, theirs))

        if change.status == "D":
            if ours_bytes is None:
                continue
            if ours is not None and ours == base:
                (git.root / ours_path).unlink()
                outcome.written.append(ours_path)
            else:
                outcome.conflicts.append(
                    f"{ours_path}: upstream deleted {change.old}, and the fork changed it"
                )
            continue

        if ours_bytes is None and change.status in "AC":
            target = new_target
            data = theirs.encode() if theirs is not None else theirs_bytes
            write(git.root / target, data)
            outcome.written.append(target)
            if name in BUILD_FILES:
                outcome.review.append(
                    f"{target}: upstream added build file {change.new}"
                )
            continue
        if ours_bytes is None:
            outcome.dropped.append(upstream_path)
            continue
        if base is None or theirs is None or ours is None:
            if ours_bytes == base_bytes:
                write(git.root / new_target, theirs_bytes)
                outcome.written.append(new_target)
            else:
                outcome.conflicts.append(
                    f"{ours_path}: binary file differs from upstream"
                )
            continue

        if name in BUILD_FILES:
            result, needs_review = port_build_file(ours, base, theirs)
            if needs_review:
                outcome.review.append(
                    f"{ours_path}: upstream changed {upstream_path} beyond its dependency lists"
                )
            conflict = False
        else:
            applied = already_applied(ours, base, theirs)
            if applied is Applied.ALL:
                continue
            if applied is Applied.SOME:
                outcome.review.append(
                    f"{ours_path}: the fork already has some of upstream's edits to {upstream_path}"
                )
            result, conflict, notes = merge(ours, base, theirs)
            outcome.notes.extend(f"{ours_path}: {note}" for note in notes)
        if new_target != ours_path:
            (git.root / ours_path).unlink()
            outcome.written.append(ours_path)
        write(git.root / new_target, result.encode())
        outcome.written.append(new_target)
        if conflict:
            outcome.conflicts.append(new_target)
    return outcome


def write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def lock_versions(text: str) -> dict[str, set[str]]:
    versions: dict[str, set[str]] = {}
    for name, version in re.findall(
        r'^name = "([^"]+)"\nversion = "([^"]+)"', text, re.MULTILINE
    ):
        versions.setdefault(name, set()).add(version)
    return versions


def update_lock_files(git: Git, commit: str) -> list[str]:
    """Replays the version changes of upstream's Cargo.lock files. Merging lock
    files line by line produces invalid files, so each package whose version
    upstream moved from one version to another moves the same way with `cargo
    update --precise`. `cargo metadata` then resolves the requirements that the
    ported `Cargo.toml` files changed, so it runs after their conflicts are
    resolved. Returns the problems found."""
    problems = []
    for upstream_path in lock_upstream_paths(git, commit):
        target = transform_path(upstream_path)
        if not (git.root / target).exists():
            continue
        before = lock_versions(decode(git.blob(f"{commit}^", upstream_path)) or "")
        after = lock_versions(decode(git.blob(commit, upstream_path)) or "")
        manifest = str(git.root / Path(target).parent / "Cargo.toml")
        for name in sorted(set(before) & set(after)):
            old, new = before[name] - after[name], after[name] - before[name]
            if len(old) != 1 or len(new) != 1:
                continue
            (old_version,), (new_version,) = old, new
            # An update can move other packages too, such as a derive crate
            # released with its crate, so the lock file is read again each time.
            ours = lock_versions((git.root / target).read_text())
            if old_version not in ours.get(name, set()):
                continue
            result = subprocess.run(
                [
                    "cargo",
                    "update",
                    "--manifest-path",
                    manifest,
                    "-p",
                    f"{name}@{old_version}",
                    "--precise",
                    new_version,
                ],
                cwd=git.root,
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode != 0:
                problems.append(
                    f"{target}: cargo update {name}@{old_version} --precise {new_version}: {result.stderr.strip()}"
                )
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", manifest],
            cwd=git.root,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            problems.append(f"{target}: cargo metadata: {result.stderr.strip()}")
        git.run("add", "--", target)
    return problems


def lock_upstream_paths(git: Git, commit: str) -> list[str]:
    return [
        c.new
        for c in upstream_changes(git, commit)
        if c.new and Path(c.new).name == "Cargo.lock"
    ]


# Lines of upstream messages that only Meta's review and export tools read.
DROPPED_LINES = re.compile(
    r"^(?:(?:Reviewed By|Differential Revision|fbshipit-source-id|Pulled By|Reviewers|Subscribers|Tags|JustKnobs|Tasks):"
    r"|bypass-github-export-checks\s*$|landed-with-\S+\s*$)",
    re.IGNORECASE,
)


def commit_message(git: Git, commit: str) -> str:
    """The message of the yak commit: the upstream message with yak names,
    without Meta's review metadata and test plan, and with the trailer that
    names the upstream commit."""
    subject = git.text("show", "-s", "--format=%s", commit).strip()
    body = git.text("show", "-s", "--format=%b", commit)
    lines = []
    for line in body.splitlines():
        if re.match(r"^Test Plan:", line):
            break
        if DROPPED_LINES.match(line):
            continue
        lines.append(re.sub(r"^Summary:\s*", "", line))
    text = "\n".join(lines).strip()
    full = git.text("rev-parse", commit).strip()
    message = rename(subject)
    if text:
        message += "\n\n" + rename(text)
    return message + f"\n\n{TRAILER}{full}\n"


def ported(git: Git) -> set[str]:
    log = git.text("log", "--format=%B", f"--grep={TRAILER}", "--fixed-strings", "HEAD")
    return set(re.findall(re.escape(TRAILER) + r"([0-9a-f]{40})", log))


def skipped() -> dict[str, str]:
    result = {}
    if SKIPPED.exists():
        for line in SKIPPED.read_text().splitlines():
            if line.strip() and not line.startswith("#"):
                commit, reason = line.split(" ", 1)
                result[commit] = reason
    return result


def pending(git: Git) -> Iterator[str]:
    done = ported(git)
    skip = skipped()
    for commit in git.text(
        "rev-list", "--reverse", "--topo-order", f"{FORK_POINT}..{UPSTREAM}"
    ).split():
        if commit not in done and commit not in skip:
            yield commit


def state_file(git: Git) -> Path:
    path = Path(git.text("rev-parse", "--git-path", "yak-port/in-progress").strip())
    return path if path.is_absolute() else git.root / path


def apply(git: Git, commit: str, auto_commit: bool) -> bool:
    """Ports one commit to the working tree and commits it when nothing needs
    attention. Returns whether it committed or skipped."""
    commit = git.text("rev-parse", "--verify", f"{commit}^{{commit}}").strip()
    if git.text(
        "status", "--porcelain", "--untracked-files=no", "--", ".", NOT_THE_LEDGER
    ).strip():
        raise SystemExit(
            "The working tree has changes. Commit or stash them before porting."
        )
    paths = build_path_map(git)
    outcome = port_files(git, commit, paths)
    subject = git.text("show", "-s", "--format=%h %s", commit).strip()
    print(f"== {subject}")
    for path in outcome.dropped:
        print(f"   dropped   {path}")
    for path in outcome.written:
        print(f"   ported    {path}")
    for note in outcome.notes:
        print(f"   note      {note}")
    for path in outcome.review:
        print(f"   REVIEW    {path}")
    for path in outcome.conflicts:
        print(f"   CONFLICT  {path}")

    if not outcome.written and not outcome.lock_files:
        areas = sorted({"/".join(Path(p).parts[:2]) for p in outcome.dropped})
        reason = "changes only files that the fork removed, in " + ", ".join(areas)
        record_skip(commit, reason)
        print(f"   skipped: {reason}")
        return True
    written = sorted(set(outcome.written) | set(outcome.lock_files))
    git.run("add", "-A", "--", *written)
    state_file(git).write_text(json.dumps({"commit": commit, "paths": written}))
    if outcome.conflicts or outcome.review or not auto_commit:
        print(
            "   Resolve the files above, then run `port.py continue`, or `port.py abort`."
        )
        return False
    finish(git, commit)
    return True


def finish(git: Git, commit: str) -> None:
    git.run("add", "-u", "--", ".", NOT_THE_LEDGER)
    unmerged = git.text(
        "diff", "--cached", "--name-only", "-G^(<<<<<<<|>>>>>>>) "
    ).split()
    if unmerged:
        raise SystemExit("Conflict markers remain in: " + ", ".join(unmerged))
    problems = update_lock_files(git, commit)
    for problem in problems:
        print(f"   PROBLEM   {problem}")
    if problems:
        raise SystemExit(
            "Fix the lock files, then run `port.py continue`, or `port.py abort`."
        )
    # The yak names sort differently from the upstream names, so the imports of
    # ported Rust files need sorting again.
    rust = [
        p
        for p in git.text("diff", "--cached", "--name-only", "--diff-filter=AM").split()
        if p.endswith(".rs")
    ]
    if rust:
        subprocess.run(["rustfmt", *rust], cwd=git.root, check=False)
        git.run("add", "--", *rust)
    if not git.text("diff", "--cached", "--name-only").strip():
        record_skip(commit, "the fork already has the change")
        print("   skipped: the fork already has the change")
    else:
        author = git.text("show", "-s", "--format=%an <%ae>", commit).strip()
        date = git.text("show", "-s", "--format=%aI", commit).strip()
        git.run(
            "commit",
            "-q",
            f"--author={author}",
            f"--date={date}",
            "-F",
            "-",
            input=commit_message(git, commit).encode(),
        )
        print("   committed " + git.text("log", "-1", "--format=%h").strip())
    state_file(git).unlink(missing_ok=True)


def record_skip(commit: str, reason: str) -> None:
    with open(SKIPPED, "a") as f:
        f.write(f"{commit} {reason}\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("fetch", help=f"fetch {UPSTREAM_URL} into the `upstream` remote")
    sub.add_parser(
        "pending", help="list the upstream commits that are neither ported nor skipped"
    )
    p = sub.add_parser(
        "apply", help="port one commit, and commit it when nothing needs attention"
    )
    p.add_argument("commit")
    p.add_argument(
        "--no-commit", action="store_true", help="stage the result without committing"
    )
    p = sub.add_parser(
        "run",
        help="port the pending commits in order, stopping at the first that needs attention",
    )
    p.add_argument("--limit", type=int, default=0, help="stop after this many commits")
    sub.add_parser("continue", help="commit the port in progress after resolving it")
    sub.add_parser("abort", help="discard the port in progress")
    p = sub.add_parser(
        "skip", help=f"record that the fork does not take a commit, in {SKIPPED.name}"
    )
    p.add_argument("commit")
    p.add_argument("reason")
    p = sub.add_parser(
        "message", help="print the yak commit message of an upstream commit"
    )
    p.add_argument("commit")
    args = parser.parse_args()

    git = Git(
        Path(
            subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip()
        )
    )
    state = state_file(git)
    if args.command in ("apply", "run") and state.exists():
        raise SystemExit(
            f"A port of {json.loads(state.read_text())['commit']} is in progress. Run `port.py continue` or `port.py abort`."
        )

    if args.command == "fetch":
        if "upstream" not in git.text("remote").split():
            git.run("remote", "add", "upstream", UPSTREAM_URL)
        git.run("fetch", "upstream", "main")
    elif args.command == "pending":
        for commit in pending(git):
            print(git.text("show", "-s", "--format=%h %as %s", commit).strip())
    elif args.command == "apply":
        sys.exit(0 if apply(git, args.commit, not args.no_commit) else 1)
    elif args.command == "run":
        for n, commit in enumerate(list(pending(git))):
            if args.limit and n >= args.limit:
                break
            if not apply(git, commit, True):
                sys.exit(1)
    elif args.command == "continue":
        if not state.exists():
            raise SystemExit("No port is in progress.")
        finish(git, json.loads(state.read_text())["commit"])
    elif args.command == "abort":
        if not state.exists():
            raise SystemExit("No port is in progress.")
        # Only the files that the port wrote are reset, so that other changes,
        # such as changes to this tool, stay.
        paths = json.loads(state.read_text())["paths"]
        git.run("reset", "-q", "HEAD", "--", *paths)
        head = git.files("HEAD")
        for path in paths:
            if path in head:
                git.run("checkout", "HEAD", "--", path)
            else:
                (git.root / path).unlink(missing_ok=True)
        state.unlink()
    elif args.command == "skip":
        commit = git.text("rev-parse", "--verify", f"{args.commit}^{{commit}}").strip()
        record_skip(commit, args.reason)
    elif args.command == "message":
        print(commit_message(git, args.commit), end="")


if __name__ == "__main__":
    main()
