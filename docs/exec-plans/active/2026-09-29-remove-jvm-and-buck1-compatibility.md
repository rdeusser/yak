# Remove the JVM, Android, JavaScript, and Buck1 compatibility code

Follows [the plan contract](../../PLANS.md).

## Purpose

The owner's projects build Go, Rust, JavaScript and TypeScript with Bun or Node, Python with uv, Dockerfiles, and C and C++ (2026-09-28).
None of them use the Java, Kotlin, or Android rules, or the React Native rules in `prelude/js`.
After the change, the prelude defines none of those rules or their toolchains, and it downloads no bootstrap jar from the upstream release.
`zip_file` builds its archive with a Python tool.
The binary drops the commands, flags, and output fields that exist only for Buck1 users, where yak has a replacement for them.

`yak targets prelude//...` in a project that vendors the prelude lists no JVM, Android, or JavaScript target.
The example projects load and build as before, and `tests/prelude/test_prelude_rules.py` builds `zip_file` targets and checks their archives.

## Progress

- [x] Inventory the JVM, Android, and JavaScript code, and the Buck1 compatibility code (2026-09-28).
- [x] The owner chooses to drop the Java, Kotlin, Android, and `prelude/js` rules (2026-09-28).
- [x] The owner chooses the Buck1 scope: the no-op code (tier A) and the compatibility surfaces that have a replacement (tier B) (2026-09-29). The tiers are defined under Context and Orientation.
- [x] Milestone 1: remove the JVM, Android, and JavaScript support (2026-09-29).
  - [x] Delete the rules, toolchains, examples, and the Java page of the website, and edit the prelude files that stay (2026-09-29).
  - [x] Replace the Java zip tool of `zip_file` with `prelude/zip_file/tools/create_zip.py`, and add `zip_file` tests to `tests/prelude/test_prelude_rules.py` (2026-09-29).
  - [x] Check the prelude with a binary built from `870fb4a2c8`, in projects that vendor the prelude before and after the change (2026-09-29). The results are under Validation and Acceptance.
  - [x] Remove the JVM code of the binary: the Android flags of `yak install`, `yak audit classpath`, the `classpath()` function of analysis queries, and the Java options of the worker protocol files (2026-09-29).
  - [x] Update the website, `CHANGELOG.md`, and the tech-debt tracker (2026-09-29). `ARCHITECTURE.md`, `AGENTS.md`, and `README.md` mention none of the removed code.
  - [x] On Linux, build the binary, run `python3 test.py` for the changed crates, and run the integration tests with regenerated golden files (2026-09-29).
  - [x] On Linux, run `python3 test.py` for the whole workspace, build the repository with yak, and build the example projects that CI builds (2026-09-29).
- [ ] Milestone 2: remove the Buck1 compatibility code of tiers A and B.

## Surprises & Discoveries

- `prelude/debugging` is the prelude half of Meta's internal debugger. `prelude/debugging/types.bzl` calls its types "a contract between a BXL script (see fdb.bxl) and internal debugging tool", and no file outside the package loads it.
- No rule in this repository creates `GraphQLInfo`, and `graphql_providers()` only forwarded the other providers of `prelude/graphql/graphql.bzl` from dependencies that had them. It returned an empty list for every rule.
- `worker_tool` was defined in `prelude/js`. It ran tools through `prelude/js/worker_runner/worker_tool_runner.py`, which exchanges the JSON messages of Buck1's worker protocol with the tool. `WorkerInfo` (see `examples/persistent_worker`) is the worker support that stays.
- `yak audit classpath` returned an error for every input before this change.
- The `classpath()` function of analysis queries read `TemplatePlaceholderInfo` keys that only the Java rules set.
- `attrs.regex()` is an alias for `attrs.string()`, so no check ran on `entries_to_exclude` before the zip tool compiled the patterns.
- The Java zip tool wrote each archive in `zip_srcs` as one run of entries, at the position of its first entry in sorted order. It wrote external attributes of 0 for the entries it copied and for directory entries, so copied executables lost their mode.
- `examples/with_prelude` fails to load until `haskell-setup.sh` creates the GHC directory. `examples/vscode` registers execution platforms only on Windows and Linux hosts, so its `cquery //...` fails on macOS. Both fail the same way before and after the change.

## Decision Log

- 2026-09-28: Remove the Java, Kotlin, and Android rules and toolchains, and the `prelude/js` rules (owner). The bootstrap jar downloads from the upstream release go with them, which supersedes the open jar item of [the rename plan](2026-09-28-rename-the-fork.md).
- 2026-09-28: Keep the `os:android` constraint. C, C++, Rust, and Go can target Android without the JVM rules. The `building_android_binary` constraint and the Android test runtimes in `prelude/runtime/constraints` go, because only the Android rules selected on them.
- 2026-09-28: `zip_file` keeps its attributes and gets a Python tool. The tool names entries, applies `entries_to_exclude`, and handles duplicates as the Java tool did. It sorts every entry by name and keeps the external attributes of entries copied from `zip_srcs`.
- 2026-09-28: Delete `prelude/debugging` and `prelude/graphql` in milestone 1. Both served only Meta's internal tools, and both held JVM code (the Java debugger branch and `GraphQLAndroidInfo`).
- 2026-09-28: Remove `worker_tool` with `prelude/js`.
- 2026-09-28: Remove the `mvn:` URLs of `remote_file`, and with them the `[http] maven_repo` and `[http] maven_repo_override` settings.
- 2026-09-29: Remove the `classpath()` function of analysis queries. `EVAL_ANALYSIS_QUERY` stops taking a `DiceComputations`, because `classpath()` was its only use of DICE.
- 2026-09-29: Leave `app/buck2_core/src/provider/flavors.rs` to milestone 2, which removes the `#flavor` syntax and with it the Android, Java, and JavaScript flavors.
- 2026-09-29: Keep the tier C behaviors (owner). They include the working directory of tests (the cell root unless `run_from_project_root` is set), the output of `audit includes`, the `%s` substitution of multiple queries, and the depth values that mean an unbounded traversal. Their Buck1 comments are prose for milestone 6 of the rename plan.

## Outcomes & Retrospective

Milestone 1 (2026-09-29):

- The prelude defines no Java, Kotlin, Android, or `prelude/js` rule. It downloads no jar. `yak targets prelude//...` lists 416 targets in place of 1173.
- `zip_file` builds its archive with `create_zip.py`, and `tests/prelude/test_prelude_rules.py` checks the archives.
- The example projects give the same results as before the change.
- The binary loses `yak audit classpath`, the `classpath()` query function, and the Android flags of `yak install`.
- `python3 test.py` runs `cargo test` without `--no-fail-fast`, so on a host where a known test fails it skips the test binaries after that one. `cargo test --workspace --no-fail-fast` ran the rest.

## Context and Orientation

[ARCHITECTURE.md](../../../ARCHITECTURE.md) describes the prelude and the crates. The binary embeds `prelude/`, so a prelude change reaches `yak` only after a rebuild.

Milestone 1 touches these areas:

- `prelude/rules_impl.bzl` registers the rule declarations of `prelude/decls/` and the implementations. `prelude/native.bzl` wraps rules in macros, and `prelude/decls/toolchains_common.bzl` gives rules their toolchain attributes.
- `prelude/cxx/`, `prelude/rust/`, and `prelude/linking/` carried Android and JVM packaging hooks (`merge_android_packageable_info`, `get_java_packaging_info`, `can_be_asset`, `include_in_android_mergemap`, and `_is_building_android_binary`).
- `prelude/zip_file/` holds the `zip_file` rule and its tools, and `prelude/toolchains/zip_file.bzl` defines its toolchain. `prelude/toolchains/demo.bzl` defines the toolchains that `system_demo_toolchains()` creates.
- `app/buck2_client/src/commands/install.rs` forwarded Android flags to the installer. `app/buck2_cmd_audit_client/` and `app/buck2_cmd_audit_server/` define the `audit` subcommands.
- `app/buck2_query_impls/src/analysis/` evaluates the queries in rule attributes (`$(query_targets ...)`), and `app/buck2_build_api/src/analysis/calculation.rs` declares the `EVAL_ANALYSIS_QUERY` hook that it implements.
- `tests/prelude/test_prelude_rules.py` builds prelude rules with `system_demo_toolchains()`. Its packages declare `py_assertion` targets that check rule outputs.

Milestone 2 works from an inventory of the Buck1 compatibility code. Each item falls into one of four tiers:

- Tier A is code that does nothing or only fails, such as hidden flags that are ignored, fields that are always false, and commands that always return an error.
- Tier B is a surface kept for Buck1 users that has a replacement in yak, such as the compatibility fields of the build report and `#flavor` labels.
- Tier C is behavior copied from Buck1 with no separate replacement, such as the working directory of tests.
- Tier D is comments and documentation that mention Buck1.

## Plan of Work

### Milestone 1

1. Delete `prelude/{java,kotlin,android,jvm,js,aosp,debugging,graphql,runtime}`, `prelude/toolchains/android/`, `prelude/toolchains/{java,kotlin,android,dex}.bzl`, `prelude/decls/{java_rules,kotlin_rules,android_rules,android_common,jvm_common,js_rules}.bzl`, `examples/android/`, `examples/with_prelude/android/`, and `website/docs/users/languages/java/`.
2. Edit the prelude files that loaded them: `rules_impl.bzl`, `native.bzl`, `decls/toolchains_common.bzl`, `decls/core_rules.bzl` (`worker_tool` and the Java examples of `http_file` and `remote_file`), `decls/uncategorized_rules.bzl` (`ndk_toolchain`), `decls/cxx_rules.bzl`, `decls/rust_rules.bzl`, `apple/apple_rules_decls.bzl`, `cxx/cxx.bzl`, `cxx/cxx_library.bzl`, `cxx/cxx_types.bzl`, `cxx/link_groups.bzl`, `cxx/prebuilt_cxx_library_group.bzl`, `rust/rust_library.bzl`, `linking/linkable_graph.bzl`, `linking/shared_libraries.bzl`, the Python extension rules, `apple/apple_library.bzl`, `genrule.bzl`, `remote_file.bzl`, `decls/remote_common.bzl`, `user/all.bzl`, `toolchains/demo.bzl`, `toolchains/conan/defs.bzl`, `ide_integrations/visual_studio/get_deps.bxl`, `os/YAK`, `YAK`, and `.yakconfig`.
3. Add `prelude/zip_file/tools/create_zip.py`, point `zip_file_toolchain` at it, and add the `zip_file` and `zip_file_errors` packages to `tests/prelude/test_prelude_rules_data/`.
4. Remove `AndroidInstallOptions` from `install.rs`, the `Classpath` audit subcommand, and the `classpath()` query function with the placeholder lookups that only it used. Delete `tests/core/analysis/test_template_placeholder*` and the classpath test of `tests/core/analysis/test_analysis_queries.py`.
5. Remove the `java_*` options from `app/buck2_worker_proto/worker.proto` and `examples/persistent_worker/proto/buck2/worker.proto`.
6. Update the website pages that document the removed rules, flags, and macros, the sidebar, `CHANGELOG.md`, and the tech-debt tracker. The tracker records that the jar item of the rename plan no longer applies, because [the plan contract](../../PLANS.md) lets a task change only its own plan.
7. Regenerate the help golden files.

### Milestone 2

Remove the tier A and tier B items of the inventory, each with its tests and documentation. The largest are the compatibility fields of the build report (`failures`, `truncated`, and the unconfigured section), which change the golden build reports and the `outputs_for_target` helper of `tests/e2e_util/api/buck_result.py`, and the `#flavor` syntax.

## Validation and Acceptance

Milestone 1, checked with a binary built from `870fb4a2c8`:

- `yak starlark typecheck` over every `.bzl` and `.bxl` file of the vendored prelude finds no type errors before the change (544 files) or after it (451 files).
- `yak targets prelude//...` succeeds before and after. It lists 1173 targets before and 416 after. The removed targets are those of the deleted packages, `prelude//:copy_android_constraint`, `prelude//os:building_android_binary`, and `prelude//os:maybe_building_android_binary`. The added target is `prelude//zip_file/tools:create_zip`.
- With the prelude vendored into each of the 15 example projects under `examples/`, `targets //...` and `cquery //...` give the same output before and after.
- In `examples/with_prelude`, `yak build //cpp/... //rust/... //python/... //go/...` succeeds before and after, and the 15 outputs are byte-identical.
- `yak build root//zip_file/...` in `tests/prelude/test_prelude_rules_data` succeeds, and each target of `root//zip_file_errors` fails with its message. Reversing the sort in `create_zip.py` makes `check_layout` and `check_merged` fail.
- On the same inputs as the Java tool, `create_zip.py` writes the same entry names and contents in the `overwrite`, `append`, and hardcoded-permission cases, and both tools fail in the `fail` case.

Milestone 1 on Linux, from the repository root (2026-09-29):

- `cargo build --bin=yak` succeeds, and `python3 test.py` passes for `buck2_analysis`, `buck2_build_api`, `buck2_client`, `buck2_cmd_audit_client`, `buck2_cmd_audit_server`, `buck2_cmd_docs_client`, `buck2_query_impls`, and `buck2_worker_proto`.
- The integration tests (`python -m pytest tests -n auto`) give 1728 passed, 223 skipped, and 3 expected failures, with no other failures. `YAK_COMPLETION_VERIFY` was unset, so the completion tests skipped. With `YAK_UPDATE_GOLDEN` set, the run changed only the help golden files of `audit` and `install`, which lose `classpath` and the Android flags.
- `git grep -n -i -E 'java_library|kotlin|android_binary|prelude//(java|android|kotlin|js)'` outside `docs/exec-plans/` and `CHANGELOG.md` finds `app/buck2_core/src/provider/flavors.rs`, which milestone 2 deletes, the language list of `website/docs/about/why.md`, and tests and parser test inputs that define rules of their own with those names.

Milestone 1 on Linux, for the whole workspace and the examples (2026-09-29):

- `python3 test.py` passes clippy and rustdoc for the whole workspace. Its unit tests stop at `test_perf_thread_instruction_counter` of `buck2_util`, which fails where the host denies `perf_event_open`.
- `cargo test --lib --workspace --no-fail-fast` runs 129 test binaries, with 3864 passed, 3 ignored, and 2 failed. The failures are `test_perf_thread_instruction_counter` and one test of `pagable::tests` in `starlark`. The tech-debt tracker records both. Milestone 1 changes neither crate. The `starlark` test passes when it runs alone.
- `cargo test --doc --workspace --no-fail-fast` gives 311 passed and 77 ignored.
- After `reindeer buckify`, `yak build //:yak` succeeds.
- In `examples/toolchains/python_toolchain`, `yak build //...` succeeds.
- In `examples/no_prelude`, `yak build //...` fails at `root//go:main`, which the tech-debt tracker records for Linux on ARM. Milestone 1 changes no file of either example.
- The integration tests of `tests/core/help`, `tests/core/docs`, `tests/prelude`, `tests/core/analysis`, `tests/core/install`, `tests/core/audit`, and `tests/core/query` give 233 passed and 28 skipped.

## Idempotence and Recovery

Each milestone is one commit on `main`. Reverting that commit restores the removed code.
The scratch projects of the checks copy the prelude, so rerunning a check needs no cleanup beyond its scratch directory.
