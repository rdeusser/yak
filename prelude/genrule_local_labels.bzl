# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//utils:selects.bzl", "selects")

"""
Handle labels used to opt-out genrules from running remotely.
"""

# Some rules have to be run locally for various reasons listed next to the label.
_GENRULE_LOCAL_LABELS = set([
    # Used for yak tests that want to run locally
    "buck2_test_local_exec",
    # Split dwarf merge rules currently don't properly list their inputs.
    "dwp",
    # Bolt and hottext post-processing rules operate on a large statically
    # linked binary which contains non-deterministic build info, meaning its
    # a) currently too large for RE to handle and b) caching it would only
    # waste cache space.
    "postprocess_bolt",
    "postprocess_hottext",
    # Gathers non-deterministic build info, such as the revision from version
    # control.
    "non_deterministic_build_info",
    # Some call "yak run" & "yak root" recursively.
    "uses_buck_run",
    # Some antlir genrules use cpio for unpacking rpms
    "uses_cpio",
    # The Antlir core compiler uses sudo
    "uses_sudo",
    # Some rules utilize hg for some reason as part of code generation
    "uses_hg",
    # Dotslash is not yet supported on RE.
    "uses_dotslash",
    # Some rules apply a patch which is not on RE.
    "uses_patch",
    # Uses shasum which is not on RE.
    "uses_shasum",
    # Uses xz which is not on RE.
    "uses_xz",
    # Uses thrift tool which is not on RE.
    "uses_thrift",
    # Uses protoc tool which is not on RE.
    "uses_protoc",
    # Uses the `libX11-devel` package which is not available on RE.
    "uses_x11",
    # Unity license client needs to be set up on RE workers for this to work, and maybe further debugging.
    "uses_unity",
    # mksquashfs isn't available in RE, so run these locally
    "uses_mksquashfs",
    # Side effecting writes directly into yak-out on the local
    # filesystem
    "writes_to_buck_out",
    # Side effecting writes directly to local filesystem outside of yak-out
    # Do not add or use in new rules, just for tagging existing rules for
    # better categorization.
    "writes_outside_buck_out",
    # Calculates and writes absolute paths in the local filesystem
    "uses_local_filesystem_abspaths",
    # Use local GPUs with latest Nvidia libs which are not available in RE yet
    "uses_lower_locally",
    # Makes recursive calls to yak
    "uses_buck",
    # Uses files in the repo that it doesn't declare as dependencies
    "uses_undeclared_inputs",
    # When run on RE produces "Cache is out of space" (excessive disk/memory)
    "re_cache_out_of_space",
    # Uses network access (unspecified what as of yet)
    "network_access",
    # Uses clang format which is not in RE
    "uses_clang_format",
    # Perform makes compilation in situ.
    "uses_make",
    # Like uses_make but for windows
    "uses_msbuild",
    # Locally built toolchains which do not exist on RE
    "toolchain_testing",
    # Uses R, which is not on RE.
    "uses_rlang",
    # Uses watchman which is not in RE
    "uses_watchman",
    # Uses yumdownloader which is not in RE
    "yumdownloader",
    # Uses locally installed mvn.
    "uses_maven",
    # Some Qt genrules don't support RE yet
    "qt_moc",
    "qt_qmlcachegen",
    "qt_qrc_compile",
    "qt_qrc_gen",
    "qt_qsb_gen",
    "qt_rcc",
    "qt_uic",
    # use local jar
    "uses_jar",
    # uses local java
    "uses_java",
    # uses ruby
    "uses_ruby",
    # Produces an output symlink. Some RE services do not return output
    # symlinks, and the symlink might not be dereferenceable anyway (for
    # example, because it points to something that does not exist).
    "dangling_output_symlink",
    # Uses Apple's codesign command which might not be in RE
    "uses_codesign",
    # The compilation databases produced by yak have paths relative to the
    # project root. This isn't compatible with RE.
    "uses_compilation_database",
    # Uses checkpolicy which is not on RE
    "uses_checkpolicy",
])

def genrule_labels_require_local(labels):
    def check_labels(labels_list):
        if labels_list == None:
            return False

        for label in labels_list:
            if selects.is_select(label):
                return selects.apply(label, lambda val: val in _GENRULE_LOCAL_LABELS if val else False)

            elif label in _GENRULE_LOCAL_LABELS:
                return True
        return False

    return selects.apply(labels, check_labels)
