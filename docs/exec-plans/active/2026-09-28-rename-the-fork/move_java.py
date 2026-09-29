#!/usr/bin/env python3
"""Milestone 4 of the rename: the Java and Kotlin packages move from
`com.facebook.buck` to `dev.yak`.

Run from the repository root of a clean checkout of the commit that finished
milestone 3, with this script and rename_runtime.py copied together outside the
checkout:

    python3 path/to/move_java.py [--apply]

Without --apply it prints what each rule would change and checks that the sort
order of the import blocks matches the order the files already have. With
--apply it moves the five source trees with `git mv`, edits the files, and sorts
the import blocks that the new names reorder. manual_edits_java.py runs next.
"""

import argparse
import collections
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rename_runtime import EXCLUDED_PREFIXES, URL, tracked_files  # noqa: E402

# Each tree moves to the directory of its new package.
MOVES = [
    (
        "prelude/toolchains/android/src/com/facebook/buck",
        "prelude/toolchains/android/src/dev/yak",
    ),
    (
        "prelude/toolchains/android/test/com/facebook/buck",
        "prelude/toolchains/android/test/dev/yak",
    ),
    (
        "prelude/toolchains/android/android/com/facebook/buck",
        "prelude/toolchains/android/android/dev/yak",
    ),
    (
        "prelude/android/tools/com/facebook/buck_generated",
        "prelude/android/tools/dev/yak_generated",
    ),
    (
        "prelude/kotlin/tools/kapt_base64_encoder/com/facebook/kapt",
        "prelude/kotlin/tools/kapt_base64_encoder/dev/yak/kapt",
    ),
]

# A name starts where no identifier character, dot, or `$` precedes it. URLs are
# skipped separately, because a URL such as
# `https://github.com/facebook/buck/blob/<rev>/src/com/facebook/buck/...` names a
# file of the upstream repository. `com.facebook.infer.annotation` keeps its
# package, and `com.facebook.buck_generated` becomes `dev.yak_generated`.
BEFORE = r"(?<![\w.$])"
RULES = [
    ("dotted buck", re.compile(BEFORE + r"com\.facebook\.buck"), "dev.yak"),
    ("slashed buck", re.compile(BEFORE + r"com/facebook/buck"), "dev/yak"),
    ("dotted kapt", re.compile(BEFORE + r"com\.facebook\.kapt(?!\w)"), "dev.yak.kapt"),
    ("slashed kapt", re.compile(BEFORE + r"com/facebook/kapt(?!\w)"), "dev/yak/kapt"),
]

# One import per line. google-java-format keeps static and other imports in
# separate blocks, and ktfmt keeps one block. Both sort a block by the imported
# name.
IMPORT = re.compile(r"import (static )?([\w.`]+(?:\.\*)?)(?: as \w+)?;?\s*$")
SORTED_SUFFIXES = (".java", ".kt")


def import_key(line):
    m = IMPORT.match(line)
    return (m.group(1) is None, m.group(2))


def import_runs(lines):
    """Yields (start, end) for each maximal run of import lines."""
    i = 0
    while i < len(lines):
        if IMPORT.match(lines[i]):
            j = i
            while j < len(lines) and IMPORT.match(lines[j]):
                j += 1
            yield i, j
            i = j
        else:
            i += 1


def sort_renamed_imports(text):
    """Sorts each import run that holds a `dev.yak` import."""
    lines = text.split("\n")
    for start, end in list(import_runs(lines)):
        run = lines[start:end]
        if any("dev.yak" in line for line in run):
            lines[start:end] = sorted(run, key=import_key)
    return "\n".join(lines)


def unsorted_runs(path, text):
    """Returns the import runs of `text` that `import_key` would reorder."""
    lines = text.split("\n")
    return [
        (path, start + 1)
        for start, end in import_runs(lines)
        if lines[start:end] != sorted(lines[start:end], key=import_key)
    ]


def rename(path, text, counts):
    urls = [m.span() for m in URL.finditer(text)]
    for name, pattern, repl in RULES:

        def sub(m, name=name, repl=repl):
            if any(a <= m.start() < b for a, b in urls):
                return m.group(0)
            counts[name] += 1
            return repl

        new = pattern.sub(sub, text)
        if new != text:
            # A replacement changes the length of the text, so find the URLs
            # again for the next rule.
            text = new
            urls = [m.span() for m in URL.finditer(text)]
    return text


def read_text(path):
    """Returns the text of `path`, or None for a binary file or a symlink."""
    if os.path.islink(path) or not os.path.isfile(path):
        return None
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
    except UnicodeDecodeError:
        return None
    # Tar archives and compiled Android XML decode as UTF-8 but hold NUL bytes,
    # and a change of length would break their headers.
    if "\0" in text:
        return None
    return text


def move_trees():
    for src, dst in MOVES:
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        subprocess.run(["git", "mv", src, dst], check=True)
        # `git mv` leaves the emptied `com/facebook` directories behind.
        parent = os.path.dirname(src)
        while parent and not os.listdir(parent):
            os.rmdir(parent)
            parent = os.path.dirname(parent)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()

    if args.apply:
        move_trees()

    counts = collections.Counter()
    changed = []
    unsorted = []
    for path in tracked_files():
        if path.startswith(EXCLUDED_PREFIXES):
            continue
        text = read_text(path)
        if text is None:
            continue
        if path.endswith(SORTED_SUFFIXES):
            unsorted += unsorted_runs(path, text)
        new = rename(path, text, counts)
        if path.endswith(SORTED_SUFFIXES):
            new = sort_renamed_imports(new)
        if new != text:
            changed.append(path)
            if args.apply:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new)

    for name, n in counts.most_common():
        print(f"{n:6d}  {name}")
    print(f"{len(changed)} files edited")
    # Before --apply, an unsorted run means that `import_key` differs from the
    # formatters' order, so sorting would reorder more than the renamed names.
    for path, line in unsorted:
        print(f"UNSORTED {path}:{line}")


if __name__ == "__main__":
    sys.exit(main())
