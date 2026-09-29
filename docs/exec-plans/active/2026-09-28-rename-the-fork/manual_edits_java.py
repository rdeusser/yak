#!/usr/bin/env python3
"""Hand edits of milestone 4 that follow move_java.py.

Run from the repository root after `move_java.py --apply`:

    python3 path/to/manual_edits_java.py

Each text edit names its file, the exact text it replaces, and how many times
that text occurs, so a rerun on a changed tree fails instead of editing blindly.
"""

import struct
import subprocess
import sys

TEST = "prelude/toolchains/android/test/dev/yak"

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


# Sizes of the constant pool entries other than CONSTANT_Utf8, by tag.
CONSTANT_SIZES = {
    3: 5, 4: 5, 5: 9, 6: 9, 7: 3, 8: 3, 9: 5, 10: 5,
    11: 5, 12: 5, 15: 4, 16: 3, 17: 5, 18: 5, 19: 3, 20: 3,
}


def replace_in_class_file(path, old, new):
    """Replaces `old` with `new` in each CONSTANT_Utf8 entry of a class file.

    Each entry stores its own length, and the rest of the file refers to entries
    by index, so an entry can change length without moving anything else.
    """
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] != b"\xca\xfe\xba\xbe":
        failures.append(f"{path}: not a class file")
        return
    count = struct.unpack(">H", data[8:10])[0]
    out = bytearray(data[:10])
    pos = 10
    index = 1
    replaced = 0
    while index < count:
        tag = data[pos]
        if tag == 1:
            length = struct.unpack(">H", data[pos + 1 : pos + 3])[0]
            value = data[pos + 3 : pos + 3 + length]
            replaced += value.count(old)
            value = value.replace(old, new)
            out += bytes([1]) + struct.pack(">H", len(value)) + value
            pos += 3 + length
        else:
            out += data[pos : pos + CONSTANT_SIZES[tag]]
            pos += CONSTANT_SIZES[tag]
            # A long or a double takes two entries.
            if tag in (5, 6):
                index += 1
        index += 1
    out += data[pos:]
    if replaced == 0:
        failures.append(f"{path}: no {old!r} in the constant pool")
        return
    with open(path, "wb") as f:
        f.write(out)


def replace_same_length(path, old, new, count=1):
    """Replaces bytes in a binary file where a change of length would break
    the file's offsets."""
    assert len(old) == len(new)
    with open(path, "rb") as f:
        data = f.read()
    n = data.count(old)
    if n != count:
        failures.append(f"{path}: expected {count} of {old!r}, found {n}")
        return
    with open(path, "wb") as f:
        f.write(data.replace(old, new))


# `resources_root` climbs from the package to `src/`, which puts the javac
# plugin jar in the package `dev.yak.jvm.java.plugin`, where `PluginLoader`
# looks for it. `dev/yak` is one directory shorter than `com/facebook/buck`.
edit(
    "prelude/toolchains/android/src/dev/yak/jvm/java/plugin/YAK",
    'resources_root = "../../../../../..",',
    'resources_root = "../../../../..",',
)

# The test data of the signature tests declares `com.example.foo`, and this
# check still named the package the data had before.
edit(
    f"{TEST}/jvm/java/abi/SignatureFactoryTest.java",
    '!signature.contains(":Lcom/facebook/foo/Dep")',
    '!signature.contains(":Lcom/example/foo/Dep")',
)

# Test inputs that stand for user code use `com.example` names.
edit(
    "prelude/android/android_instrumentation_test.bzl",
    "(e.g. com.facebook.R$style)",
    "(e.g. com.example.R$style)",
)
# The compiled layout names a custom view of the app. The new name has the same
# length, which keeps the offsets of the string pool.
replace_same_length(
    f"{TEST}/android/resources/testdata/aapt_dump/row_with_button.xml",
    b"com.facebook.buck.PageInfo",
    b"com.example.views.PageInfo",
)

# `ZipOutputStreamTest` compresses this class file of Buck1 as sample input.
replace_in_class_file(
    f"{TEST}/util/zip/sample-bytes.dat",
    b"com/facebook/buck/",
    b"dev/yak/",
)

# No test reads the archives, and `standard_java_test` packages `testdata/`
# only for targets that set `with_test_data`. `move_java.py` staged their move,
# so `git rm` needs `-f`.
subprocess.run(
    ["git", "rm", "-q", "-r", "-f", f"{TEST}/util/unarchive/testdata"],
    check=True,
)

# The status lines name the work that remains.
edit(
    "AGENTS.md",
    "tracks the remaining work, such as the Java packages and the `buck2*` crates.",
    "tracks the remaining work, such as the `buck2*` crates.",
)
edit(
    "AGENTS.md",
    "such as release downloads and the `com.facebook` packages of the JVM toolchain.",
    "such as release downloads.",
)
edit(
    "ARCHITECTURE.md",
    """- The rename to yak continues with the Java packages and the `buck2*` crates. `docs/exec-plans/active/2026-09-28-rename-the-fork.md` tracks the work.
- The `com.facebook` packages of the JVM and Android toolchain will move to a package under the new name. `docs/exec-plans/tech-debt-tracker.md` lists them with the other upstream connections that remain.
""",
    """- The rename to yak continues with the `buck2*` crates. `docs/exec-plans/active/2026-09-28-rename-the-fork.md` tracks the work.
- The bootstrap jars of the JVM toolchain will be built from this repository's sources and stored outside the upstream releases. `docs/exec-plans/tech-debt-tracker.md` lists them with the other upstream connections that remain.
""",
)

edit(
    "CHANGELOG.md",
    """- The integration tests take the binary from `YAK_BINARY` and rewrite golden files when `YAK_UPDATE_GOLDEN` is set.
""",
    """- The integration tests take the binary from `YAK_BINARY` and rewrite golden files when `YAK_UPDATE_GOLDEN` is set.
- The Java and Kotlin packages of the JVM and Android toolchain are under `dev.yak` in place of `com.facebook.buck`, such as `dev.yak.jvm.java`.
- Apps that use exopackage extend `dev.yak.android.support.exopackage.ExopackageApplication`.
- The JUnit runner reads its log levels from the system properties `dev.yak.stdOutLogLevel` and `dev.yak.stdErrLogLevel`.
- The source ABI and KSP steps pass the annotation processor options `dev.yak.java.generating_abi`, `dev.yak.kotlin.generating_abi`, and `dev.yak.kotlin.ksp_generated_out_path`.
- The Java classes of the worker protocol are in `dev.yak.worker.model`.
""",
)

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("manual edits applied")
