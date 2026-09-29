#!/usr/bin/env python3
"""Milestone 3 of the rename: environment variables, configuration sections, and flags.

Run from the repository root of a clean checkout of the commit that finished
milestones 1 and 2, with this script and rename_runtime.py copied together
outside the checkout:

    python3 path/to/rename_env.py [--apply]

Without --apply it prints what each rule would change. With --apply it edits the
files. manual_edits_env.py runs next.
"""

import argparse
import collections
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rename_runtime import (  # noqa: E402
    EXCLUDED_PREFIXES,
    URL,
    rust_segments,
    tracked_files,
)

# Names that look like environment variables but name a constant, a field, an
# enum value, or a placeholder. Identifiers keep their names in this milestone.
KEEP = frozenset(
    {
        # Rust constants and statics.
        "BUCK2_BUILD_INFO",
        "BUCK2_RE_CLIENT_CFG_SECTION",
        "BUCK2_TEST_EXECUTOR_USER_ENV_VAR",
        "BUCK2_TEST_HOME_DIR_ENV_VAR",
        "BUCK2_WRAPPER_ENV_VAR",
        "BUCKD_PREV_DIR",
        "BUCK_AUTH_TOKEN_HEADER",
        "BUCK_PNG",
        "BUCK_WRAPPER_START_TIME_ENV_VAR",
        "BUCK_WRAPPER_UUID_ENV_VAR",
        # Protobuf enum values.
        "BUCKD_EXE_DELETED",
        "BUCKD_INFO_MISSING",
        "BUCKD_INFO_PARSE_ERROR",
        "BUCKD_LIFECYCLE_LOCK",
        "BUCK_VERSION_ERROR",
        # Python, Starlark, and Java constants.
        "BUCK2_BINARY_ENV_VAR",
        "BUCK_OUTPUT_PATH_DEFAULT",
        "BUCK_OUT_ROOT_REL_PATH",
        "BUCK_PYTHON_RULE_KINDS",
        "BUCK_PYTHON_RULE_KIND_QUERY",
        # The placeholder that the integration tests put in place of the path of
        # the daemon's info file.
        "BUCKD_INFO",
    }
)

ENV_VAR = re.compile(r"(?<![A-Za-z0-9_])(?:BUCK2|BUCKD|BUCK)_[A-Z0-9_]+(?![A-Za-z0-9_])")


def new_env_name(name):
    """Returns the yak name of an environment variable."""
    if name.startswith("BUCKD_"):
        new = "YAKD_" + name[len("BUCKD_") :]
    elif name.startswith("BUCK2_"):
        new = "YAK_" + name[len("BUCK2_") :]
    else:
        new = "YAK_" + name[len("BUCK_") :]
    # `BUCK2_NO_BUCKD` and `BUCK2_TEST_FAIL_BUCKD_AUTH` name the daemon.
    return new.replace("_BUCKD", "_YAKD")


# The configuration sections other than `[buck2]`. No crate or target has these
# names, so the rules can match them without a list of keys.
SECTION_SUFFIXES = "re_client|resource_control|system_warning|hydration|metadata"

# Words after `buck2.` that are not configuration keys: type-id domains, a
# protocol marker, the protobuf package, and file names.
NOT_KEYS = (
    "builtin_provider",
    "data",
    "provider",
    "provider_singleton",
    "rs",
    "structured-dep-file-inputs",
    "transitive_set",
    "user_provider",
)


class Rule:
    def __init__(self, name, pattern, repl, whole_text=False, only=None, skip=None, skip_basenames=()):
        self.name = name
        self.re = re.compile(pattern)
        self.repl = repl
        # whole_text: in a Rust file, the rule matches across code and strings,
        # because its pattern includes the code around a string literal. Every
        # other rule applies only inside Rust strings and comments. No rule of
        # this milestone renames a Rust identifier.
        self.whole_text = whole_text
        self.only = only
        self.skip = skip or ()
        self.skip_basenames = skip_basenames

    def applies_to(self, path):
        if self.only is not None and not any(path.startswith(p) or path.endswith(p) for p in self.only):
            return False
        if os.path.basename(path) in self.skip_basenames:
            return False
        return not any(path.startswith(p) for p in self.skip)


RULES = [
    Rule("env var", ENV_VAR.pattern, lambda m: m.group(0) if m.group(0) in KEEP else new_env_name(m.group(0))),
    # The completion scripts pass the binary through this variable.
    Rule("_BUCK_COMPLETE_BIN", r"\b_BUCK_COMPLETE_BIN\b", "_YAK_COMPLETE_BIN"),
    # Section headers, in configuration files and in text that quotes them. A
    # Markdown link whose text is `[buck2]` names the tool, not a section.
    Rule(
        "[buck2] header",
        r"\[buck2(_(?:" + SECTION_SUFFIXES + r"))?\](?!\()",
        r"[yak\1]",
    ),
    # Section names in code: Rust key structs, Starlark reads, and the literals
    # of the sections that only configuration uses.
    Rule("section field", r'("?section"?\]?\s*(?::|==)\s*)"buck2"', r'\1"yak"', whole_text=True),
    # The integration tests pass configuration as a dictionary of sections.
    Rule("config dict", r'(?<![A-Za-z0-9_])"buck2"(\s*:\s*\{)', r'"yak"\1', only=(".py",)),
    Rule("static section", r'(static_str!\(SECTION_BUCK2\s*=\s*)"buck2"', r'\1"yak"', whole_text=True),
    Rule("read_config", r'(read_(?:root_)?config\(\s*)"buck2(_(?:' + SECTION_SUFFIXES + r'))?"', r'\1"yak\2"'),
    # `buck2_resource_control` is also a crate, which milestone 5 renames.
    Rule(
        "section literal",
        r'"buck2_(' + SECTION_SUFFIXES + r')"',
        r'"yak_\1"',
        skip_basenames=("Cargo.toml", "YAK"),
    ),
    # Keys written as `section.key`, as `-c` takes them and as the
    # documentation names them.
    Rule(
        "section.key",
        r"(?<![A-Za-z0-9_\-/.:@])buck2(_(?:" + SECTION_SUFFIXES + r"))?\.(?!(?:" + "|".join(re.escape(w) for w in NOT_KEYS) + r")(?![A-Za-z0-9_\-]))(?!workspace\b)([a-z_][a-z0-9_]*)",
        r"yak\1.\2",
    ),
    # The flag that runs the daemon in the client process.
    Rule("--no-buckd", r"--no-buckd\b", "--no-yakd"),
]

# Reported, not replaced: each needs a decision about its meaning.
MANUAL = [
    ('exact "buck2" literal', re.compile(r"(?<![A-Za-z0-9_])[\"']buck2[\"']")),
    ("buck2.key", re.compile(r"(?<![A-Za-z0-9_\-/.:@])buck2\.[a-z_]+")),
    ("buckd flag", re.compile(r"no[-_]buckd", re.I)),
    ("host_info buck2", re.compile(r"host_info\(\)\.buck2\b|\"buck2\", Value::new_bool")),
]


def apply_rules(path, text, counts, samples):
    rules = [r for r in RULES if r.applies_to(path)]
    if not rules:
        return text

    def sub(rule, segment):
        urls = [m.span() for m in URL.finditer(segment)]

        def repl(m):
            if any(a <= m.start() < b for a, b in urls):
                return m.group(0)
            new = rule.repl(m) if callable(rule.repl) else m.expand(rule.repl)
            if new != m.group(0):
                counts[rule.name] += 1
                if len(samples[rule.name]) < 600:
                    samples[rule.name].append((path, m.group(0), new))
            return new

        return rule.re.sub(repl, segment)

    if path.endswith(".rs"):
        for rule in rules:
            if rule.whole_text:
                text = sub(rule, text)
        parts = []
        for is_code, seg in rust_segments(text):
            if not is_code:
                for rule in rules:
                    if not rule.whole_text:
                        seg = sub(rule, seg)
            parts.append(seg)
        return "".join(parts)
    for rule in rules:
        text = sub(rule, text)
    return text


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
                manual.append((name, path, line, m.group(0)))
        if new != text:
            changed.append(path)
            if args.apply:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new)

    for name, n in counts.most_common():
        print(f"{n:6d}  {name}")
    print(f"{len(changed)} files edited")
    for name, path, line, text in manual:
        print(f"MANUAL {name}: {path}:{line}: {text}")
    if args.samples:
        with open(args.samples, "w") as f:
            for name, items in samples.items():
                f.write(f"== {name}\n")
                for path, old, new in items:
                    f.write(f"{path}\t{old}\t{new}\n")


if __name__ == "__main__":
    sys.exit(main())
