#!/usr/bin/env python3
"""Milestone 6 of the rename, second part: the messages and comments of the code.

Run from the repository root of a clean checkout of the commit before the
change, with this script copied outside the checkout, under Python 3.12 or
later (the tokenizer splits f-strings from 3.12 on):

    python3 path/to/rename_code_prose.py [--apply]

Without --apply it prints what each rule would change and the lines that still
name Buck for the hand edits. With --apply it edits the files, and
manual_edits_code.py runs next.

The rules edit only comments and string literals. A tokenizer finds them in
Rust, Python, Starlark, and C-style sources, so identifiers such as the `buck`
fixture of the integration tests and the `Buck` struct of rust-project keep
their names.
"""

import argparse
import collections
import io
import os
import re
import subprocess
import sys
import tokenize

# The golden files hold the output of the binary, and the integration tests
# regenerate them. games/ names its deer character Buck. The parser test cases
# of starlark_syntax are copies of other projects' files.
SKIPPED = (
    "docs/exec-plans/",
    "third-party/",
    "website/",
    ".claude/",
    "games/",
    "CHANGELOG.md",
    "starlark-rust/starlark_syntax/testcases/",
)
GOLDEN = re.compile(r"(^|/)[^/]*golden[^/]*/|\.golden(\.|$)")

# Test data under tests/ can compare its strings with output, so only its
# comments change.
TEST_DATA = re.compile(r"^tests/.*_data/")

RUST = (".rs",)
PYTHON = (".py", ".bzl", ".bxl", ".star")
PYTHON_NAMES = ("YAK", "PACKAGE", "YAK.fixture", "YAK.test", "YAK.missing")
C_STYLE = (".proto", ".go", ".c", ".cc", ".cpp", ".h", ".hpp", ".m", ".mm")

URL = re.compile(r"https?://[^\s)\]>\"'`]+")
INLINE_CODE = re.compile(r"(`+)(?:(?!\1).)+?\1")

# Phrases that credit the upstream project keep the name Buck2.
CREDITS = re.compile(r"fork of (?:Meta's )?Buck2|forked from Buck2|Buck2, which Meta created")

# Characters around a word that make it part of an identifier, a path, a label,
# an attribute such as `buck.type`, or the `#[buck2(...)]` attribute. A period
# ends a sentence before whitespace or a closing quote.
BEFORE = r"(?<![\w$@/\\.:\-#\[{])"
AFTER = r"(?![\w\-/\\:}\]\(]|\.[^\s\"'])"

COMMANDS = (
    "build|test|run|install|query|uquery|cquery|aquery|targets|utargets|ctargets|"
    "kill|killall|clean|log|bxl|audit|init|help|debug|starlark|docs|rage|profile|"
    "explain|status|subscribe|completion|daemon|root|server|lsp|expand-external-cell"
)

RULES = [
    # Commands, also inside inline code.
    ("commands", True, re.compile(r"(?<![\w$./\\-])buck2? (?=(?:" + COMMANDS + r")\b)"), "yak "),
    ("Buck2", False, re.compile(BEFORE + r"Buck2" + AFTER), "yak"),
    ("Buck-out", False, re.compile(r"(?<![\w$])Buck-out(?![\w])"), "yak-out"),
    # The hand edits rewrite the phrases that expand BXL, name Buck1, or name
    # the Buck UI.
    (
        "Buck",
        False,
        re.compile(BEFORE + r"Buck" + AFTER + r"(?!\s?[vV]?[12]\b| [Ee]xtension [Ll]anguage| UI\b)"),
        "yak",
    ),
    ("buck2", False, re.compile(BEFORE + r"buck2" + AFTER), "yak"),
    ("buck", False, re.compile(BEFORE + r"buck" + AFTER + r"(?!\s?[vV]?[12]\b)"), "yak"),
    (
        "buckconfig",
        False,
        re.compile(r"(?<![\w$.\-/\\:])[Bb]uckconfig(?=s?(?![\w/]|::|\.[^\s\"']))"),
        "yakconfig",
    ),
]

# A quoted name inside a longer string is a value, such as the field that a
# test checks for, or the `buckconfig` section of `prelude/utils/buckconfig.bzl`.
QUOTED_NAME = re.compile(r"""(["'])(?:[Bb]uck2?|buckconfig)\.?\1""")

# A string literal that holds only the name is a value, such as a program name
# or a cell name. The hand edits decide each one.
EXACT_LITERAL = re.compile(r"""^[bru]*(['"])(?:buck2?|Buck2?)\1$""")

# Lines that still name Buck after the rules, for the hand edits: the words
# that the rules skip in inline code or before the exceptions, Buck1, and
# BUCK2. Labels, paths, and identifiers that contain the name are left out.
REMAINING = re.compile(
    BEFORE
    + r"(?:Buck2?|buck2?)"
    + AFTER
    + r"|(?<![\w$])(?:BUCK2|[Bb]uck ?[vV]?1|[Bb]uckv1)(?![\w])"
    + r"|(?<![\w$.\-/\\:])[Bb]uckconfigs?(?![\w/]|::|\.\w)"
)

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


def c_style_spans(text):
    """Returns (start, end, kind) of the comments and strings of Rust or C-style source."""
    spans = []
    for m in RUST_TOKEN.finditer(text):
        if m.lastgroup in ("lc", "bc"):
            spans.append((m.start(), m.end(), "comment"))
        elif m.lastgroup in ("raw", "str"):
            spans.append((m.start(), m.end(), "string"))
    return spans


def python_spans(text):
    """Returns (start, end, kind) of the comments and strings of Python or Starlark source."""
    starts = [0]
    for line in text.splitlines(keepends=True):
        starts.append(starts[-1] + len(line))
    spans = []
    fstring = getattr(tokenize, "FSTRING_MIDDLE", None)
    for tok in tokenize.generate_tokens(io.StringIO(text).readline):
        if tok.type == tokenize.COMMENT:
            kind = "comment"
        elif tok.type == tokenize.STRING or (fstring is not None and tok.type == fstring):
            kind = "string"
        else:
            continue
        start = starts[tok.start[0] - 1] + tok.start[1]
        end = starts[tok.end[0] - 1] + tok.end[1]
        spans.append((start, end, kind))
    return spans


def protected_spans(segment):
    spans = [m.span() for m in URL.finditer(segment)]
    spans += [m.span() for m in CREDITS.finditer(segment)]
    spans += [m.span() for m in QUOTED_NAME.finditer(segment)]
    return spans


def edit_segment(segment, counts):
    for name, in_code, pattern, repl in RULES:
        spans = protected_spans(segment)
        if not in_code:
            spans += [m.span() for m in INLINE_CODE.finditer(segment)]
        out = []
        last = 0
        for m in pattern.finditer(segment):
            if any(start <= m.start() < end for start, end in spans):
                continue
            out.append(segment[last : m.start()])
            out.append(repl)
            last = m.end()
            counts[name] += 1
        if out:
            out.append(segment[last:])
            segment = "".join(out)
    return segment


def tracked_files():
    out = subprocess.run(
        ["git", "-c", "submodule.recurse=false", "ls-files", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    return [p.decode() for p in out.split(b"\0") if p]


def spans_of(path, text):
    base = os.path.basename(path)
    if path.endswith(RUST) or path.endswith(C_STYLE):
        return c_style_spans(text)
    if path.endswith(PYTHON) or base in PYTHON_NAMES:
        return python_spans(text)
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    counts = collections.Counter()
    changed = 0
    remaining = []
    literals = []
    for path in tracked_files():
        if path.startswith(SKIPPED) or GOLDEN.search(path) or os.path.islink(path):
            continue
        if not os.path.isfile(path):
            continue
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except UnicodeDecodeError:
            continue
        spans = spans_of(path, text)
        if spans is None:
            continue
        test_data = bool(TEST_DATA.search(path))
        parts = []
        last = 0
        for start, end, kind in spans:
            segment = text[start:end]
            parts.append(text[last:start])
            if kind == "string" and EXACT_LITERAL.match(segment):
                literals.append((path, text.count("\n", 0, start) + 1, segment))
            elif not (kind == "string" and test_data):
                segment = edit_segment(segment, counts)
            parts.append(segment)
            last = end
        parts.append(text[last:])
        new_text = "".join(parts)
        for start, end, kind in spans_of(path, new_text):
            segment = new_text[start:end]
            if kind == "string" and (test_data or EXACT_LITERAL.match(segment)):
                continue
            unprotected = segment
            for s, e in sorted(protected_spans(unprotected), reverse=True):
                unprotected = unprotected[:s] + unprotected[e:]
            for m in REMAINING.finditer(unprotected):
                line_no = new_text.count("\n", 0, start) + unprotected.count("\n", 0, m.start()) + 1
                line = new_text.split("\n")[line_no - 1]
                remaining.append(f"{path}:{line_no}: {line.strip()[:160]}")
        if new_text != text:
            changed += 1
            if args.apply:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new_text)

    for name, count in sorted(counts.items()):
        print(f"{count:6d}  {name}")
    print(f"{changed} files changed")
    print(f"{len(literals)} string literals that hold only the name:")
    for path, line, segment in literals:
        print(f"  {path}:{line}: {segment}")
    remaining = list(dict.fromkeys(remaining))
    print(f"{len(remaining)} lines still name Buck:")
    for line in remaining:
        print("  " + line)


if __name__ == "__main__":
    sys.exit(main())
