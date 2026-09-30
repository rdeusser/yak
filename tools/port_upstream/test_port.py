#!/usr/bin/env python3
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import unittest

import port


class RenameTest(unittest.TestCase):
    def test_upstream_names_take_the_yak_form_in_the_same_case(self) -> None:
        self.assertEqual(
            "use yak_core::YakError; YAK_LOG yak-out .yakconfig Yak",
            port.rename(
                "use buck2_core::Buck2Error; BUCK2_LOG buck-out .buckconfig Buck"
            ),
        )

    def test_words_that_start_with_bucket_keep_their_names(self) -> None:
        self.assertEqual("bucket Buckets", port.rename("bucket Buckets"))

    def test_text_about_the_upstream_project_keeps_its_names(self) -> None:
        text = "See https://github.com/facebook/buck2/issues/1, buck.build, and Buck1."
        self.assertEqual(text, port.rename(text))

    def test_labels_of_meta_repository_name_this_repository(self) -> None:
        self.assertEqual(
            '"//third-party/rust:tokio", "//app/yak_core:yak_core"',
            port.rename(
                '"fbsource//third-party/rust:tokio", "//buck2/app/buck2_core:buck2_core"'
            ),
        )


class OpenSourceSideTest(unittest.TestCase):
    def test_keeps_enabled_lines_and_drops_disabled_lines(self) -> None:
        text = (
            "a = 1\n"
            "# @oss-disable[end= ]: b = 2\n"
            "    // @oss-disable: internal();\n"
            'c = "11", # @oss-enable\n'
        )
        self.assertEqual('a = 1\nc = "11",\n', port.open_source_side(text))


YAK_FILE = """load("//build_defs:rust.bzl", "rust_library")

rust_library(
    name = "yak_test",
    deps = [
        "//app/yak_core:yak_core",
        "//third-party/rust:serde",
        "//third-party/rust:tokio",
    ],
)
"""

UPSTREAM_BEFORE = """load("@fbsource//tools/build_defs:rust_library.bzl", "rust_library")

oncall("build_infra")

rust_library(
    name = "yak_test",
    deps = [
        "//third-party/rust:serde",
        "//third-party/rust:tokio",
        "//app/yak_core:yak_core",
    ],
)
"""


class BuildFileTest(unittest.TestCase):
    def test_dependency_changes_reach_the_same_list_in_sorted_order(self) -> None:
        after = UPSTREAM_BEFORE.replace(
            '        "//third-party/rust:tokio",\n',
            '        "//third-party/rust:itertools",\n',
        )
        result, needs_review = port.port_build_file(YAK_FILE, UPSTREAM_BEFORE, after)
        self.assertFalse(needs_review)
        self.assertEqual(
            YAK_FILE.replace(
                '        "//third-party/rust:serde",\n        "//third-party/rust:tokio",\n',
                '        "//third-party/rust:itertools",\n        "//third-party/rust:serde",\n',
            ),
            result,
        )

    def test_other_changes_need_review(self) -> None:
        after = UPSTREAM_BEFORE.replace('oncall("build_infra")', 'oncall("other")')
        result, needs_review = port.port_build_file(YAK_FILE, UPSTREAM_BEFORE, after)
        self.assertTrue(needs_review)
        self.assertEqual(YAK_FILE, result)

    def test_a_list_the_yak_file_lacks_needs_review(self) -> None:
        after = UPSTREAM_BEFORE.replace(
            "    deps = [",
            '    test_deps = [\n        "//third-party/rust:maplit",\n    ],\n    deps = [',
        )
        result, needs_review = port.port_build_file(YAK_FILE, UPSTREAM_BEFORE, after)
        self.assertTrue(needs_review)
        self.assertEqual(YAK_FILE, result)


class PathMapTest(unittest.TestCase):
    def path_map(self) -> port.PathMap:
        head = {
            "app/yak_core/src/lib.rs",
            "website/docs/index.md",
            "prelude/rust/build.bzl",
        }
        fork = {
            "app/yak_core/src/lib.rs",
            "docs/index.md",
            "prelude/java/java.bzl",
            "prelude/rust/build.bzl",
        }
        moved = {"docs/index.md": "website/docs/index.md"}
        return port.PathMap(
            head_files=head,
            moved=moved,
            removed={"prelude/java/java.bzl"},
            dirs={"docs": "website/docs"},
            head_dirs=port.dirs_of(head),
            fork_dirs=port.dirs_of(fork),
        )

    def test_a_file_the_fork_kept_takes_its_yak_path(self) -> None:
        self.assertEqual(
            "app/yak_core/src/lib.rs",
            self.path_map().target("app/buck2_core/src/lib.rs"),
        )

    def test_a_file_the_fork_moved_takes_its_new_path(self) -> None:
        self.assertEqual(
            "website/docs/index.md", self.path_map().target("docs/index.md")
        )

    def test_a_new_file_goes_where_the_fork_moved_its_directory(self) -> None:
        self.assertEqual("website/docs/new.md", self.path_map().target("docs/new.md"))

    def test_a_new_file_in_a_kept_directory_keeps_its_path(self) -> None:
        self.assertEqual(
            "app/yak_core/src/new.rs",
            self.path_map().target("app/buck2_core/src/new.rs"),
        )

    def test_files_in_removed_directories_are_dropped(self) -> None:
        self.assertIsNone(self.path_map().target("prelude/java/java.bzl"))
        self.assertIsNone(self.path_map().target("prelude/java/new.bzl"))

    def test_a_new_directory_is_kept(self) -> None:
        self.assertEqual(
            "app/yak_new/src/lib.rs", self.path_map().target("app/buck2_new/src/lib.rs")
        )


class LockVersionsTest(unittest.TestCase):
    def test_lists_each_version_of_a_package(self) -> None:
        text = '[[package]]\nname = "syn"\nversion = "1.0.1"\n\n[[package]]\nname = "syn"\nversion = "2.0.3"\n'
        self.assertEqual({"syn": {"1.0.1", "2.0.3"}}, port.lock_versions(text))


if __name__ == "__main__":
    unittest.main()
