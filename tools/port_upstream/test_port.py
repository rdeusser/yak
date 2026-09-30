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

    def test_calls_named_like_the_upstream_site_take_yak_names(self) -> None:
        self.assertEqual("await yak.build(", port.rename("await buck.build("))

    def test_labels_of_meta_repository_name_this_repository(self) -> None:
        self.assertEqual(
            '"//third-party/rust:tokio", "//app/yak_core:yak_core"',
            port.rename(
                '"fbsource//third-party/rust:tokio", "//buck2/app/buck2_core:buck2_core"'
            ),
        )


class ForkNamesTest(unittest.TestCase):
    def test_soft_errors_take_the_fork_option_name(self) -> None:
        self.assertEqual("hard_error: true", port.rename("error_on_oss: true"))

    def test_test_decorators_drop_arguments_the_fork_lacks(self) -> None:
        self.assertEqual(
            '@yak_test()\n@yak_test(skip_for_os=["windows"])\nfileinput.input(p, inplace=True)\n',
            port.transform(
                "@buck_test(setup_eden=False)\n"
                '@buck_test(inplace=False, skip_for_os=["windows"])\n'
                "fileinput.input(p, inplace=True)\n"
            ),
        )

    def test_linter_markers_are_dropped(self) -> None:
        self.assertEqual(
            "x = 1\nimport a\n",
            port.rename(
                "# @nolint\nx = 1  # pyre-ignore[16]\n    # pyre-fixme[6]: why\nimport a\n"
            ),
        )

    def test_meta_names_take_the_fork_names(self) -> None:
        self.assertEqual(
            "build_info YAK_TEST_EXECUTOR_USE_TCP",
            port.rename("fb_build_info BUCK2_TEST_TPX_USE_TCP"),
        )

    def test_re_metadata_takes_no_fbcode_argument(self) -> None:
        self.assertEqual(
            "with_re_metadata(req, metadata\n)",
            port.rename(
                "with_re_metadata(req, metadata, self.runtime_opts.use_fbcode_metadata,\n)"
            ),
        )

    def test_re_platforms_are_cloned(self) -> None:
        self.assertEqual(
            "Some(platform.clone()), ctx.re_platform()",
            port.rename("Some(re_platform(platform)), ctx.re_platform()"),
        )

    def test_settings_take_the_open_source_default(self) -> None:
        self.assertEqual(
            "    default: Some(false),\n",
            port.rename(
                "    internal_default: Some(true),\n    oss_default: Some(false),\n"
            ),
        )

    def test_review_references_are_dropped(self) -> None:
        self.assertEqual(
            "on a large analysis when",
            port.rename("on a large analysis (D66773980) when"),
        )

    def test_golden_headers_take_the_fork_wording(self) -> None:
        self.assertEqual(
            "regenerate by rerunning the test with `YAK_UPDATE_GOLDEN=1` set",
            port.rename(
                "regenerate by re-running test with `-- --env BUCK2_UPDATE_GOLDEN=1` appended to the test command"
            ),
        )

    def test_integration_tests_take_the_fork_imports(self) -> None:
        self.assertEqual(
            "# licenses.\n\nfrom e2e_util.api.yak import Yak\n",
            port.rename(
                "# licenses.\n\n# pyre-strict\n\n\nfrom buck2.tests.e2e_util.api.buck import Buck\n"
            ),
        )


class TransformPathTest(unittest.TestCase):
    def test_targets_files_take_the_yak_build_file_name(self) -> None:
        self.assertEqual(
            "tests/a_data/YAK.fixture",
            port.transform_path("tests/a_data/TARGETS.fixture"),
        )
        self.assertEqual(
            "app/yak_core/YAK", port.transform_path("app/buck2_core/TARGETS")
        )
        self.assertEqual(
            "templates/TARGETS_BIN", port.transform_path("templates/TARGETS_BIN")
        )

    def test_fixture_names_in_contents_take_the_yak_name(self) -> None:
        self.assertEqual(
            'yak.cwd / "YAK.fixture"', port.rename('buck.cwd / "TARGETS.fixture"')
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

    def test_a_list_that_continues_with_a_select_takes_the_changes(self) -> None:
        yak_file = YAK_FILE.replace("    ],\n)", '    ] + select({"DEFAULT": []}),\n)')
        after = UPSTREAM_BEFORE.replace('        "//third-party/rust:tokio",\n', "")
        result, needs_review = port.port_build_file(yak_file, UPSTREAM_BEFORE, after)
        self.assertFalse(needs_review)
        self.assertEqual(
            yak_file.replace('        "//third-party/rust:tokio",\n', ""), result
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


MANIFEST = """[package]
name = "yak_test"

[dependencies]
serde.workspace = true
tokio.workspace = true
yak_core.workspace = true

[lints]
workspace = true
"""


class CargoManifestTest(unittest.TestCase):
    def test_dependency_changes_reach_the_manifest(self) -> None:
        after = UPSTREAM_BEFORE.replace(
            '        "//third-party/rust:tokio",\n',
            '        "//third-party/rust:itertools",\n',
        )
        yak_file, _ = port.port_build_file(YAK_FILE, UPSTREAM_BEFORE, after)
        result, problems = port.port_cargo_manifest(
            MANIFEST, UPSTREAM_BEFORE, after, yak_file, {"itertools", "serde", "tokio"}
        )
        self.assertEqual([], problems)
        self.assertEqual(
            MANIFEST.replace(
                "serde.workspace = true\ntokio.workspace = true\n",
                "itertools.workspace = true\nserde.workspace = true\n",
            ),
            result,
        )

    def test_a_dependency_the_workspace_lacks_is_a_problem(self) -> None:
        after = UPSTREAM_BEFORE.replace(
            '        "//third-party/rust:tokio",\n',
            '        "//third-party/rust:tokio",\n        "//third-party/rust:unknown",\n',
        )
        yak_file, _ = port.port_build_file(YAK_FILE, UPSTREAM_BEFORE, after)
        result, problems = port.port_cargo_manifest(
            MANIFEST, UPSTREAM_BEFORE, after, yak_file, {"serde", "tokio"}
        )
        self.assertEqual(MANIFEST, result)
        self.assertEqual(1, len(problems))


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


class MergeTest(unittest.TestCase):
    def test_edits_to_adjacent_lines_combine(self) -> None:
        base = "a\nhyper = 1\nhyperlocal = 1\nz\n"
        ours = "a\nhyper = 1\nz\n"
        theirs = "a\nhyper = 2\nhyperlocal = 1\nz\n"
        self.assertEqual(
            ("a\nhyper = 2\nz\n", False, []), port.merge(ours, base, theirs)
        )

    def test_edits_to_the_same_line_conflict(self) -> None:
        base = "a\nb\nc\n"
        result, conflict, _ = port.merge("a\nB\nc\n", base, "a\nbb\nc\n")
        self.assertTrue(conflict)
        self.assertIn("<<<<<<< yak", result)

    def test_insertions_at_the_same_line_conflict(self) -> None:
        base = "a\nc\n"
        _, conflict, _ = port.merge("a\nours\nc\n", base, "a\ntheirs\nc\n")
        self.assertTrue(conflict)

    def test_the_fork_deletion_of_changed_lines_stays(self) -> None:
        base = "a\nfn internal() {\n    old();\n}\nz\n"
        theirs = "a\nfn internal() {\n    new();\n}\nz\n"
        result, conflict, notes = port.merge("a\nz\n", base, theirs)
        self.assertEqual(("a\nz\n", False), (result, conflict))
        self.assertEqual(1, len(notes))

    def test_additions_in_lines_the_fork_deleted_conflict(self) -> None:
        base = "a\n# old\nz\n"
        theirs = "a\n# new\nimport json\nz\n"
        _, conflict, notes = port.merge("a\nz\n", base, theirs)
        self.assertTrue(conflict)
        self.assertEqual([], notes)

    def test_replacements_apply_where_their_lines_occur_once(self) -> None:
        base = ["a\n", "old\n", "b\n"]
        upstream = port.edits(base, ["a\n", "new\n", "b\n"])
        self.assertEqual(
            ["x\n", "new\n", "y\n"],
            port.apply_edits_by_content(["x\n", "old\n", "y\n"], base, upstream),
        )
        self.assertIsNone(
            port.apply_edits_by_content(["old\n", "old\n"], base, upstream)
        )

    def test_edits_of_lines_the_fork_deleted_are_left_out(self) -> None:
        base = ["a\n", "old\n", "b\n", "gone\n"]
        upstream = port.edits(base, ["a\n", "new\n", "b\n", "changed\n"])
        self.assertEqual(
            ["x\n", "new\n", "b\n"],
            port.apply_edits_by_content(["x\n", "old\n", "b\n"], base, upstream),
        )

    def test_edits_apply_to_a_pruned_file_by_content(self) -> None:
        base = "".join(f"line {i}\n" for i in range(40))
        theirs = base.replace("line 30\n", "line thirty\n")
        ours = "".join(f"line {i}\n" for i in range(0, 40, 3)).replace(
            "line 3\n", "line three\n"
        )
        result, conflict, _ = port.merge(ours, base, theirs)
        self.assertFalse(conflict)
        self.assertIn("line thirty\n", result)
        self.assertIn("line three\n", result)

    def test_use_runs_sort_before_a_merge(self) -> None:
        base = port.sort_use_runs("use a::B;\nuse a::Old;\nuse a::C;\n")
        ours = port.sort_use_runs("use a::B;\nuse a::C;\nuse a::Old;\n")
        theirs = port.sort_use_runs("use a::B;\nuse a::New;\nuse a::C;\n")
        self.assertEqual(
            ("use a::B;\nuse a::C;\nuse a::New;\n", False, []),
            port.merge(ours, base, theirs),
        )


class SpellingTest(unittest.TestCase):
    def test_lines_take_the_fork_spelling_of_the_name(self) -> None:
        ours = 'msg("yak daemon is busy")\n'
        base = 'msg("Yak daemon is busy")\n'
        theirs = 'msg("Yak daemon is busy")\nnew()\n'
        self.assertEqual(
            (ours, ours + "new()\n"), port.adopt_fork_spelling(ours, base, theirs)
        )


class DependencyRunsTest(unittest.TestCase):
    def test_dependency_tables_take_the_fork_order_before_a_merge(self) -> None:
        ours = "[dependencies]\nanyhow.workspace = true\nyak_core.workspace = true\nbytes.workspace = true\n"
        base = port.order_dependency_runs(
            "[dependencies]\nyak_core.workspace = true\nanyhow.workspace = true\nbytes.workspace = true\n",
            ours,
        )
        theirs = port.order_dependency_runs(
            "[dependencies]\nyak_core.workspace = true\nanyhow.workspace = true\nbytes.workspace = true\nstrong_hash.workspace = true\n",
            ours,
        )
        self.assertEqual(
            (
                "[dependencies]\nanyhow.workspace = true\nstrong_hash.workspace = true\nyak_core.workspace = true\nbytes.workspace = true\n",
                False,
                [],
            ),
            port.merge(ours, base, theirs),
        )


class MetaMarkersTest(unittest.TestCase):
    def test_lines_for_meta_builds_are_found(self) -> None:
        self.assertEqual(
            {"#[cfg(not(fbcode_build))]"},
            port.meta_markers("#[cfg(not(fbcode_build))]\nuse tracing::instrument;\n"),
        )


class PlausibleMovesTest(unittest.TestCase):
    def test_moves_of_unrelated_files_out_of_deleted_directories_are_deletions(
        self,
    ) -> None:
        fork = ["shim/a.bzl", "shim/config.bzl", "shim/b.bzl", "docs/x.md", "docs/y.md"]
        moved = {
            "shim/config.bzl": "tests/data/rules/config.bzl",
            "docs/x.md": "website/docs/x.md",
            "docs/y.md": "website/docs/yak_y.md",
        }
        kept, removed = port.plausible_moves(moved, {"shim/a.bzl", "shim/b.bzl"}, fork)
        self.assertEqual(
            {"docs/x.md": "website/docs/x.md", "docs/y.md": "website/docs/yak_y.md"},
            kept,
        )
        self.assertEqual({"shim/a.bzl", "shim/b.bzl", "shim/config.bzl"}, removed)

    def test_unrelated_names_are_not_moves(self) -> None:
        kept, removed = port.plausible_moves(
            {"tests/a/fixups.toml": "tests/b/prelude.bzl"},
            set(),
            ["tests/a/fixups.toml"],
        )
        self.assertEqual(({}, {"tests/a/fixups.toml"}), (kept, removed))

    def test_dot_files_compare_whole_names(self) -> None:
        self.assertTrue(port.related_names(".gitignore", ".gitignore"))
        self.assertFalse(port.related_names(".yakroot", ".yakconfig"))

    def test_directories_that_the_fork_mostly_deleted(self) -> None:
        fork = ["shim/a", "shim/b", "shim/c/d", "keep/e", "keep/f"]
        self.assertEqual(
            {"shim", "shim/c"},
            port.removed_directories(fork, {"shim/a", "shim/c/d", "keep/e"}),
        )


class LockVersionsTest(unittest.TestCase):
    def test_lists_each_version_of_a_package(self) -> None:
        text = '[[package]]\nname = "syn"\nversion = "1.0.1"\n\n[[package]]\nname = "syn"\nversion = "2.0.3"\n'
        self.assertEqual({"syn": {"1.0.1", "2.0.3"}}, port.lock_versions(text))


if __name__ == "__main__":
    unittest.main()
