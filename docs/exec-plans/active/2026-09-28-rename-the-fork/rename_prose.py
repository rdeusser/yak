#!/usr/bin/env python3
"""Milestone 6 of the rename, first part: the documentation and the website.

Run from the repository root of a clean checkout of the commit before the
change, with this script copied outside the checkout:

    python3 path/to/rename_prose.py [--apply]

Without --apply it prints what each rule would change and the lines that still
name Buck for the hand edits. With --apply it moves and deletes the files in
MOVES and DELETIONS with Git and then edits the files. The hand edits in the
same commit rewrite what the rules leave: the pages about Buck1, headings, the
talk transcript, and the diagrams.
"""

import argparse
import collections
import os
import re
import subprocess
import sys

MOVES = {
    ".claude/skills/buck2-rule-basics": ".claude/skills/yak-rule-basics",
    "website/docs/buck2_lab": "website/docs/yak_lab",
    "website/docs/concepts/buck_out.md": "website/docs/concepts/yak_out.md",
    "website/docs/concepts/buck_query_language.md": "website/docs/concepts/query_language.md",
    "website/docs/concepts/buckconfig.md": "website/docs/concepts/yakconfig.md",
    "website/docs/getting_started/what_is_buck2.md": "website/docs/getting_started/what_is_yak.md",
    "website/docs/users/faq/buck_hanging.md": "website/docs/users/faq/yak_hanging.md",
}

# The page that compares Buck2 with Buck1, and the images that show Buck names.
# Mermaid diagrams in the hand edits replace the images.
DELETIONS = [
    "website/docs/about/benefits/compared_to_buck1.md",
    "website/static/img/buck2_architecture.png",
    "website/static/img/buck2_conceptmap.png",
    "website/static/img/buck2_rule_workflow.png",
    "website/static/img/packages-1.png",
    "website/static/img/target_node_label_relationship.png",
]

# The hand edits own these files. The transcript keeps the speaker's words, and
# CHANGELOG.md keeps the history of upstream releases. The golden files hold the
# docs that the binary generates from its doc comments, so they change with the
# code in the second part of the milestone.
SKIPPED = (
    "docs/exec-plans/",
    "third-party/",
    "CHANGELOG.md",
    "website/docs/insights_and_knowledge/modern_dice.md",
    "tests/core/docs/test_builtin_docs_data/",
)

WEBSITE_SOURCES = (
    "website/config_impl.ts",
    "website/sidebars.ts",
    "website/redirects.ts",
    "website/gen_docs.py",
)


URL = re.compile(r"https?://[^\s)\]>\"'`]+")

# Phrases that credit the upstream project keep the name Buck2.
CREDITS = re.compile(
    r"fork of (?:Meta's )?Buck2|Based on Buck2|forked from Buck2|Buck2, which Meta created"
    r"|Using buck to build Rust projects"
)

INLINE_CODE = re.compile(r"(`+)(?:(?!\1).)+?\1")
FENCE = re.compile(r"^\s*(```+|~~~+)\s*([\w-]*)")

# A rule for prose applies only to Markdown files, and it skips inline code and
# fenced code blocks, where `Buck` can name a Python class and `buck.type`
# names an attribute. Mermaid diagrams count as prose.
PROSE = "prose"
ANY = "any"


def anchor(m):
    """Renames the words of a link's fragment that a renamed heading changed."""
    words = ["yak" if w in ("buck", "buck2") else w for w in m.group(2).split("-")]
    return m.group(1) + "-".join(words)


# `(?<![\w$])` and `(?![\w])` keep the rules out of identifiers such as
# `buck2_core`, `Buck2TestHeapName`, and `LegacyBuckConfig`.
RULES = [
    # Links and ids of the moved pages, the skill directory, and the directory
    # of the skill's tutorial.
    ("page ids", ANY, re.compile(r"what_is_buck2"), "what_is_yak"),
    ("page ids", ANY, re.compile(r"buck2_lab"), "yak_lab"),
    ("page ids", ANY, re.compile(r"(?<=[/('\"])buck_out(?=\.md|[/#)'\"]|$)"), "yak_out"),
    ("page ids", ANY, re.compile(r"^id: buck_out$"), "id: yak_out"),
    ("page ids", ANY, re.compile(r"buck_query_language"), "query_language"),
    ("page ids", ANY, re.compile(r"buck_hanging"), "yak_hanging"),
    ("page ids", ANY, re.compile(r"buck2-rule-basics"), "yak-rule-basics"),
    ("page ids", ANY, re.compile(r"buck2-tutorial"), "yak-tutorial"),
    # A heading that names the tool gets a new anchor.
    ("anchors", ANY, re.compile(r"(\]\([^)\s#]*#)([\w-]+)"), anchor),
    # The name of the tool. In code blocks it appears in comments and strings,
    # and the word boundaries keep it out of identifiers.
    ("Buck2", ANY, re.compile(r"(?<![\w$@/])Buck2(?![\w])"), "yak"),
    # Buck without a version names the tool in prose ("the Buck daemon"). The
    # hand edits rewrite the phrases that expand BXL and name Buck1.
    (
        "Buck",
        PROSE,
        re.compile(
            r"(?<![\w$@/])Buck(?![\w]|-out|\s?[vV]?[12]\b| [eE]xtension| Query Language)"
        ),
        "yak",
    ),
    ("Buck-out", PROSE, re.compile(r"(?<![\w$])Buck-out(?![\w])"), "yak-out"),
    # Lowercase names of the tool in prose ("the buck daemon"). Paths, labels,
    # and attribute keys such as `buck.type` keep the name.
    ("buck", PROSE, re.compile(r"(?<![\w$@/.:-])buck2?(?![\w./:-]|\s?[vV]?[12]\b)"), "yak"),
    # Commands that start with the name of a binary other than `yak`.
    (
        "commands",
        ANY,
        re.compile(
            r"(?<![\w$./-])buck2? (?=(?:build|test|run|install|query|uquery|cquery|aquery|"
            r"targets|utargets|ctargets|kill|clean|log|bxl|audit|init|help|debug|"
            r"starlark|docs|rage|profile|explain|status|subscribe|completion)\b)"
        ),
        "yak ",
    ),
    # The configuration files are `.yakconfig` files, so their contents are the
    # yakconfig. `.buckconfig` itself was renamed in milestone 2.
    ("buckconfig", PROSE, re.compile(r"(?<![\w$.])[Bb]uckconfig(?=s?(?![\w]))"), "yakconfig"),
]

# Lines that still name Buck after the rules, for the hand edits.
REMAINING = re.compile(
    r"(?<![\w$])(?:Buck2?|buck2?|BUCK2?)(?![\w])|[Bb]uck ?v?[12](?![\w])|[Bb]uckconfig"
)


def tracked_files():
    out = subprocess.run(
        ["git", "-c", "submodule.recurse=false", "ls-files", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    return [p.decode() for p in out.split(b"\0") if p]


def in_scope(path):
    if path.startswith(SKIPPED) or os.path.islink(path):
        return False
    if path in WEBSITE_SOURCES or path.startswith(("website/src/", "website/docs/", ".claude/")):
        return True
    return path.endswith((".md", ".mdx"))


def moved(path):
    """Returns the path of a file after the moves."""
    for old, new in MOVES.items():
        if path == old or path.startswith(old + "/"):
            return new + path[len(old) :]
    return path


def protected_spans(line, code):
    spans = [m.span() for m in URL.finditer(line)]
    spans += [m.span() for m in CREDITS.finditer(line)]
    if code:
        spans += [m.span() for m in INLINE_CODE.finditer(line)]
    return spans


def edit_line(line, in_code_block, markdown, counts):
    for name, scope, pattern, repl in RULES:
        if scope == PROSE and (in_code_block or not markdown):
            continue
        spans = protected_spans(line, scope == PROSE)
        out = []
        last = 0
        for m in pattern.finditer(line):
            if any(start <= m.start() < end for start, end in spans):
                continue
            new = repl(m) if callable(repl) else m.expand(repl)
            out.append(line[last : m.start()])
            out.append(new)
            last = m.end()
            if new != m.group(0):
                counts[name] += 1
        if out:
            out.append(line[last:])
            line = "".join(out)
    return line


def edit_text(text, markdown, counts):
    lines = []
    fence = None
    for line in text.split("\n"):
        m = FENCE.match(line) if markdown else None
        if m and fence is None:
            fence = (m.group(1)[0], m.group(2))
            lines.append(line)
            continue
        if m and fence is not None and m.group(1)[0] == fence[0] and not m.group(2):
            fence = None
            lines.append(line)
            continue
        in_code_block = fence is not None and fence[1] != "mermaid"
        lines.append(edit_line(line, in_code_block, markdown, counts))
    return lines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    if args.apply:
        for old, new in MOVES.items():
            subprocess.run(["git", "mv", old, new], check=True)
        subprocess.run(["git", "rm", "-q", *DELETIONS], check=True)

    # Without --apply the files keep their old paths, and the rules still see
    # each file once. The files in DELETIONS are left out, so both modes count
    # the same replacements.
    counts = collections.Counter()
    changed = 0
    remaining = []
    for path in tracked_files():
        if path in DELETIONS or not in_scope(moved(path)) or not os.path.isfile(path):
            continue
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except UnicodeDecodeError:
            continue
        lines = edit_text(text, path.endswith((".md", ".mdx")), counts)
        for number, line in enumerate(lines, 1):
            unprotected = line
            for start, end in sorted(protected_spans(line, False), reverse=True):
                unprotected = unprotected[:start] + unprotected[end:]
            if REMAINING.search(unprotected):
                remaining.append(f"{path}:{number}: {line.strip()[:160]}")
        new_text = "\n".join(lines)
        if new_text != text:
            changed += 1
            if args.apply:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new_text)

    for name, count in sorted(counts.items()):
        print(f"{count:6d}  {name}")
    print(f"{changed} files changed")
    print(f"{len(remaining)} lines still name Buck:")
    for line in remaining:
        print("  " + line)


if __name__ == "__main__":
    sys.exit(main())
