#!/usr/bin/env python3
"""Milestones 1 and 2 of the rename: runtime file, directory, and process names.

Run from the repository root of a clean checkout of commit b25e8f97d8, with the
script copied outside the checkout:

    python3 path/to/rename_runtime.py [--apply]

Without --apply it prints what each rule would change. With --apply it edits the
files and runs `git mv` for the renamed files. manual_edits.py runs next.
"""

import argparse
import collections
import os
import re
import subprocess
import sys

# Files and directories the script never edits. The plans, the tracker, and the
# changelog record history, and their current-state lines are edited by hand.
EXCLUDED_PREFIXES = (
    "docs/exec-plans/",
    "CHANGELOG.md",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "NOTICE",
    "Cargo.lock",
)

# Word boundaries for names that must not match inside identifiers, paths, or
# config keys.
W_BEFORE = r"(?<![A-Za-z0-9_$\-])"
W_AFTER = r"(?![A-Za-z0-9_\-])"


class Rule:
    def __init__(self, name, pattern, repl, rust_code=False, only=None, skip=None):
        self.name = name
        self.re = re.compile(pattern)
        self.repl = repl
        # rust_code: the rule also applies to Rust code outside strings and
        # comments. Only the rules that rename constants set it, because a file
        # name in Rust code is always a string, and `.buckconfig` outside one is
        # a field access.
        self.rust_code = rust_code
        self.only = only
        self.skip = skip or ()

    def applies_to(self, path):
        if self.only is not None and not any(path.startswith(p) for p in self.only):
            return False
        return not any(path.startswith(p) for p in self.skip)


JAVA_TREES = (
    "prelude/toolchains/android/",
    "prelude/android/tools/com/",
    "prelude/kotlin/tools/kapt_base64_encoder/",
)

RULES = [
    # Output directory.
    Rule("buck-out", r"buck-out", "yak-out"),
    # Configuration files.
    Rule(".buckconfig", r"\.buckconfig(?![A-Za-z0-9_])", ".yakconfig"),
    Rule(".buckconfigs", r"\.buckconfigs\b", ".yakconfigs", skip=("app/",)),
    Rule(".buckconfig_no_external", r"\.buckconfig_no_external\b", ".yakconfig_no_external"),
    Rule("/etc/buckconfig", r"/etc/buckconfig", "/etc/yakconfig"),
    Rule(
        "ProgramData buckconfig",
        r"(ProgramData(?:\\\\|\\|/))buckconfig",
        r"\1yakconfig",
    ),
    Rule(
        "config constants",
        r"\bDOT_BUCKCONFIG_(D|LOCAL)\b",
        r"DOT_YAKCONFIG_\1",
        rust_code=True,
    ),
    # Root marker and settings files.
    Rule(".buckroot", r"\.buckroot", ".yakroot"),
    Rule(".bucksettings", r"\.bucksettings", ".yaksettings"),
    Rule(
        "settings constants",
        r"\bDOT_BUCKSETTINGS(_LOCAL)?\b",
        r"DOT_YAKSETTINGS\1",
        rust_code=True,
    ),
    # Home directory `~/.buck`.
    Rule(
        "home .buck",
        r"(?<![A-Za-z0-9_.])\.buck(?=[/\\\"'`)\s]|$)",
        ".yak",
        skip=JAVA_TREES,
    ),
    # Daemon directory and its files.
    Rule("BUCKD_LIFECYCLE", r"\bBUCKD_LIFECYCLE\b", "YAKD_LIFECYCLE", rust_code=True),
    Rule(
        "buckd",
        r"(?<![A-Za-z0-9_\-])buckd(?![A-Za-z0-9_])",
        "yakd",
        skip=JAVA_TREES,
    ),
    # Test fixtures took Meta's `TARGETS` build file name.
    Rule("TARGETS.fixture", r"\bTARGETS\.fixture\b", "YAK.fixture"),
    Rule("TARGETS.test", r"\bTARGETS\.test\b", "YAK.test"),
    # Package file alternative name.
    Rule("BUCK_TREE", r"\bBUCK_TREE\b", "YAK_TREE"),
    # Build files. `BUCK.v2` is reported for manual editing, see MANUAL below.
    Rule(
        "BUCK",
        W_BEFORE + r"BUCK" + W_AFTER + r"(?!\.v2)",
        "YAK",
        skip=JAVA_TREES,
    ),
    # The binary.
    Rule(
        "cargo bin",
        r"(--bin[= ]|target/(?:[a-z0-9_\-]+/)?(?:debug|release)/)buck2(?=[\s\"'`),;:]|$)",
        r"\1yak",
    ),
    Rule("buck2.exe", r"(?<![A-Za-z0-9_\-/])buck2\.exe\b", "yak.exe"),
    # The repository's own binary target.
    Rule("//:buck2", r"(?<![A-Za-z0-9_])//:buck2(?![A-Za-z0-9_\-])", "//:yak"),
    Rule("buck2-daemon", r"(?<![A-Za-z0-9_\-])buck2-daemon\b", "yak-daemon"),
    Rule("buck2d", r"(?<![A-Za-z0-9_\-])buck2d\b", "yakd"),
    Rule("buck2 slice", r"\bbuck2\.slice\b", "yak.slice"),
    Rule("buck2_daemon.scope", r"\bbuck2_daemon\.scope\b", "yak_daemon.scope"),
    Rule("--slice=buck2", r"--slice=buck2\b", "--slice=yak"),
    # The command. A bare lowercase `buck2` word is the command or the binary.
    # Section names (`[buck2]`), keys (`buck2.x`), paths, crates, and targets
    # are excluded by the surrounding characters, and exact "buck2" string
    # literals are reported for manual editing. `$` matches at the end of each
    # line. `cargo install --git <url> buck2` names the Cargo package, which
    # milestone 5 renames.
    Rule(
        "buck2 command",
        r"(?m)(?<![A-Za-z0-9_\-/.:@\[#<\"'=])(?<!\.git )buck2(?=[ `'\),;!?]|\.(?:\s|$)|$)(?!'s\b)(?!\s*=)",
        "yak",
        # The root build file and `defs.bzl` use `buck2` as a Starlark name.
        skip=JAVA_TREES + ("BUCK", "defs.bzl"),
    ),
    Rule("buck2's", r"(?<![A-Za-z0-9_\-/.:@\[#<\"'=])buck2's\b", "yak's", skip=JAVA_TREES),
    Rule(
        "quoted buck2 command",
        r"(?<=[`\"'])buck2(?= [a-z\-])",
        "yak",
        skip=JAVA_TREES,
    ),
]

URL = re.compile(r"https?://[^\s)\]>\"'`]+")

RUST_TOKEN = re.compile(
    r"""
    (?P<lc>//[^\n]*)
  | (?P<bc>/\*.*?\*/)
  | (?P<raw>b?r(?P<h>\#*)".*?"(?P=h))
  | (?P<chr>b?'(?:[^'\\\n]|\\.[^'\n]{0,9})')
  | (?P<str>b?"(?:[^"\\]|\\.)*")
    """,
    re.S | re.X,
)

# Reported, not replaced: each needs a decision about its meaning.
MANUAL = [
    ("BUCK.v2", re.compile(r"BUCK\.v2")),
    ('exact "buck2" literal', re.compile(r"(?<![A-Za-z0-9_])[\"']buck2[\"']")),
    ("buckd flag", re.compile(r"no-buckd")),
]


def rust_segments(text):
    """Yields (is_code, segment) pairs that together make up `text`."""
    pos = 0
    for m in RUST_TOKEN.finditer(text):
        if m.start() > pos:
            yield True, text[pos : m.start()]
        yield False, m.group(0)
        pos = m.end()
    if pos < len(text):
        yield True, text[pos:]


def tracked_files():
    out = subprocess.run(
        ["git", "-c", "submodule.recurse=false", "ls-files", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    return [p.decode() for p in out.split(b"\0") if p]


def apply_rules(path, text, counts, samples):
    rules = [r for r in RULES if r.applies_to(path)]
    if not rules:
        return text

    def sub(rule, segment):
        urls = [m.span() for m in URL.finditer(segment)]

        def repl(m):
            if any(a <= m.start() < b for a, b in urls):
                return m.group(0)
            counts[rule.name] += 1
            if len(samples[rule.name]) < 400:
                samples[rule.name].append((path, m.group(0)))
            return m.expand(rule.repl)

        return rule.re.sub(repl, segment)

    if path.endswith(".rs"):
        parts = []
        for is_code, seg in rust_segments(text):
            for rule in rules:
                if is_code and not rule.rust_code:
                    continue
                seg = sub(rule, seg)
            parts.append(seg)
        return "".join(parts)
    for rule in rules:
        text = sub(rule, text)
    return text


# Files renamed with `git mv`, by base name.
RENAMED_BASENAMES = {
    "BUCK": "YAK",
    ".buckroot": ".yakroot",
    "BUCK_TREE": "YAK_TREE",
    "BUCK.fixture": "YAK.fixture",
    "BUCK.missing": "YAK.missing",
    "BUCK.example": "YAK.example",
    "TARGETS.fixture": "YAK.fixture",
    "TARGETS.test": "YAK.test",
}


def renamed_basename(base):
    """Returns the new base name of a renamed file, or None."""
    if base in RENAMED_BASENAMES:
        return RENAMED_BASENAMES[base]
    # `.buckconfig`, `.buckconfig.local`, and the alternative configurations
    # that projects swap in, such as `.buckconfig.no-workers`.
    if base.startswith(".buckconfig"):
        return ".yakconfig" + base[len(".buckconfig") :]
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--samples", default=None, help="write rule samples to this file")
    args = ap.parse_args()

    counts = collections.Counter()
    samples = collections.defaultdict(list)
    manual = []
    changed = []
    for path in tracked_files():
        if path.startswith(EXCLUDED_PREFIXES) or os.path.islink(path):
            continue
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
            continue
        new = apply_rules(path, text, counts, samples)
        for name, pat in MANUAL:
            for m in pat.finditer(new):
                line = new.count("\n", 0, m.start()) + 1
                manual.append((name, path, line))
        if new != text:
            changed.append(path)
            if args.apply:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new)

    renames = []
    for path in tracked_files():
        if path.startswith(EXCLUDED_PREFIXES):
            continue
        new_base = renamed_basename(os.path.basename(path))
        if new_base is not None:
            renames.append((path, os.path.join(os.path.dirname(path), new_base)))
    if args.apply:
        for src, dst in renames:
            subprocess.run(["git", "mv", src, dst], check=True)

    for name, n in counts.most_common():
        print(f"{n:6d}  {name}")
    print(f"{len(changed)} files edited, {len(renames)} files renamed")
    for name, path, line in manual:
        print(f"MANUAL {name}: {path}:{line}")
    if args.samples:
        with open(args.samples, "w") as f:
            for name, items in samples.items():
                f.write(f"== {name}\n")
                for path, s in items:
                    f.write(f"{path}\t{s}\n")


if __name__ == "__main__":
    sys.exit(main())
