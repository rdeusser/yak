# Rename the fork, its files, and its Java packages

Follows [the plan contract](../../PLANS.md).

## Purpose

The fork is an internal tool of the owner's company named yak.
After the change, the binary is `yak`, it reads `YAK` files and `.yakconfig` files, it writes `yak-out`, and no source file uses a `com.facebook` Java package other than the Infer annotations (see Decision Log).
`yak init` in an empty directory writes a `.yakconfig` file, and `git grep -E '(^|[^.[:alnum:]_])com[./]facebook'` finds only the Infer annotation imports outside `docs/exec-plans/`.

## Progress

- [x] The owner picks the name: yak (2026-09-28).
- [x] The owner chooses the root of the Java packages: `dev.yak` (2026-09-28).
- [x] The owner decides whether the tool keeps reading `BUCK` and `.buckconfig` files: it does not (2026-09-28).
- [x] The owner decides whether the Cargo packages and directories named `buck2*` take the new name: they do (2026-09-28).
- [x] Milestone 1: the binary uses the new names for its files and directories (2026-09-28).
- [x] Milestone 2: the repository's own build files and the example projects use `YAK` and `.yakconfig` (2026-09-28).
  - `rename_runtime.py` and `manual_edits.py` in `docs/exec-plans/active/2026-09-28-rename-the-fork/` made both milestones from commit `b25e8f97d8`. `rename_runtime.py` edited 1374 files and renamed 1979. `cargo fmt --all` and the golden files that the integration tests regenerated complete the change.
  - On Linux, `cargo build --bin=yak` and `cargo fmt --all -- --check` pass. `python3 test.py` passes clippy and rustdoc. `cargo test --lib --no-fail-fast` passes 3865 unit tests and fails `test_perf_thread_instruction_counter`, which fails wherever `perf_event_open` is denied (see the tech-debt tracker). `cargo test --doc` passes 311 doc tests.
  - On Linux, the integration tests give 1763 passed, 190 skipped, and 3 expected failures, with `BUCK2_COMPLETION_VERIFY` set so the completion tests run in bash, fish, and zsh.
  - On Linux, `target/debug/yak init --git <dir>` writes `.yakconfig`, `.yakroot`, and a `.gitignore` that lists `/yak-out`.
  - On Linux, after `./bootstrap/reindeer --third-party-dir third-party/rust buckify`, `yak build //:yak`, `yak build //app_dep_graph_rules:test_buck2_dep_graph`, and `yak targets //...` succeed. `yak bxl prelude//rust/rust-analyzer/resolve_deps.bxl:resolve_targets -- --targets //app/buck2_wrapper_common:buck2_wrapper_common` succeeds and reports source folders in `yak-out`.
  - On Linux, `yak build //... -v 2` succeeds in `examples/toolchains/python_toolchain`. In `examples/no_prelude`, it fails only at the Go download that the tech-debt tracker lists under "Examples that fail to load or build".
  - On Linux, in a project that `yak init` created and whose build files are named `BUILD`, `resolve_deps.bxl:resolve_targets` reports `<project root>/lib` as the source folder of a Windows-only `rust_library` in `lib/BUILD`.
  - On macOS, `cargo build --bin=yak` passes, and `python3 test.py` passes clippy and rustdoc. `cargo test --lib --no-fail-fast` passes 3860 unit tests and fails one of the paging tests of `starlark`, which fail when they run in parallel (see the tech-debt tracker). `cargo test --doc` passes 311 doc tests.
  - On macOS, `yak init --git <dir>` writes `.yakconfig`, `.yakroot`, and a `.gitignore` that lists `/yak-out`. `resolve_deps.bxl:resolve_targets` reports `<project root>/lib` as the source folder of the Windows-only crate in `lib/BUILD`.
  - Remaining on macOS: the integration tests and the Buck build of the repository, which the tech-debt tracker lists as failing.
- [ ] The owner decides whether the default isolation dir keeps the name `v2`, which puts build output in `yak-out/v2/`.
- [x] Milestone 3: the environment variables take the `YAK_` prefix, and the configuration sections and the flag that say `buck2` or `buckd` take `yak` (2026-09-28).
  - `rename_env.py` and `manual_edits_env.py` in `docs/exec-plans/active/2026-09-28-rename-the-fork/` made the milestone from commit `04fcc47b52`. `rename_env.py` edited 814 files. `cargo fmt --all` and the golden files that the integration tests regenerated complete the change.
  - On Linux, `cargo build --bin=yak` and `cargo fmt --all -- --check` pass. `python3 test.py` passes clippy and rustdoc. `cargo test --lib --no-fail-fast` passes 3865 unit tests and fails only `test_perf_thread_instruction_counter`, and `cargo test --doc` passes 311 doc tests.
  - On Linux, the integration tests gave 1760 passed, 190 skipped, 3 expected failures, and 3 failures. `yak help-env` sorts the variables by name, and a build report cuts an error message at a fixed length, so three golden files changed with the names. `test_external_buckconfigs` expected `buck2` to sort before its own section. After the regenerated golden files and the fixed test, `tests/core/help`, `test_build_report_errors.py`, and `test_external_buckconfigs.py` give 39 passed and 3 skipped.
  - On Linux, `yak init --git`, the `resolve_deps.bxl` check, and the two example projects give the same results as for milestone 2, and the integration tests left 13 action processes running, as before.
  - On Linux, after `buckify`, `yak build //:yak`, `yak build //app_dep_graph_rules:test_buck2_dep_graph`, `yak targets //...`, and the `resolve_deps.bxl` run of the repository succeed. The repository's proto rules pass `YAK_PROTO_SRCS` to the build scripts.
- [ ] Milestone 4: the Java packages move under the chosen root, and the four bootstrap jars are rebuilt from the moved sources.
  - [x] Test inputs that stand for user code use `com.example` names, which needs no package root (2026-09-28).
    - A script replaced 394 names in 22 files under `prelude/toolchains/android/test/` and `prelude/android/tools/tests/`. It left the packages that this repository declares outside `testdata/` and the Infer package.
    - By hand, the escaped patterns in `ClassNameFilterTest` changed, two expected carets in `InterfaceValidatorTest` moved one column, and `JavaInMemoryFileObjectTest` uses `com/example/app/java/` paths.
    - The JUnit reports of `prelude//toolchains/android/test/com/facebook/buck/jvm/java/abi/...`, `jvm/java:java`, `android/dex:dex`, `android/apkmodule:apkmodule`, and `testrunner:per_test_coverage_base_test` list the same 1645 passes and 36 failures before and after, with the test JVM run with `-Dnet.bytebuddy.experimental=true`. The 36 failures are those under "JVM tests fail in their JUnit reports" in the tech-debt tracker.
    - `python3 -m unittest android.tools.tests.test_sort_pre_dexed_files`, run from `prelude/`, passes its 26 tests.
- [ ] Milestone 5: the Cargo packages and directories take the new name.
- [ ] Milestone 6: the documentation and the website use the new name.

## Surprises & Discoveries

- `com.facebook` names more than this repository's packages (2026-09-28). The Java and Kotlin sources outside test data declare 104 packages under it, and eight `.proto` files set a `java_package` under it. The other names fall into four groups:
  - The Infer annotation jar defines `com.facebook.infer.annotation`, which 132 files import.
  - Test inputs stand for user code, such as `com.facebook.bar` in the ABI tests and `com.facebook.base.data.json` in `CopyResourcesStepTest`.
  - The JUnit runner reads its log levels from the system properties `com.facebook.buck.stdOutLogLevel` and `com.facebook.buck.stdErrLogLevel`.
  - The source ABI and KSP steps pass the options `com.facebook.buck.java.generating_abi`, `com.facebook.buck.kotlin.generating_abi`, and `com.facebook.buck.kotlin.ksp_generated_out_path` to annotation processors, and nothing in this repository reads them.
- A replacement of every `com.facebook` string would break the Infer imports, because the jar keeps its package.
- The Java and Kotlin sources live in five trees, which hold 701, 329, 9, 1, and 1 files: `prelude/toolchains/android/src/com/facebook/`, `prelude/toolchains/android/test/com/facebook/`, `prelude/toolchains/android/android/com/facebook/`, `prelude/android/tools/com/facebook/buck_generated/`, and `prelude/kotlin/tools/kapt_base64_encoder/com/facebook/kapt/`.
- Apps compile the exopackage support classes in `prelude/toolchains/android/android/com/facebook/` and extend `ExopackageApplication`, so moving that package changes a name that app code uses.
- `yak test` passes a JVM test target whatever its JUnit results (tech-debt tracker, "`yak test` reports failing JVM tests as passing"). The JUnit reports are the evidence for milestone 4.
- The ABI tests compare compiler diagnostics that print a caret under the error column, so a package name of a different length moves the caret. `JavaInMemoryFileObjectTest` counts the directory entries of a jar, so a path of a different depth changes the count.
- `prelude/toolchains/android/test/com/facebook/buck/util/unarchive/testdata/generate_archives.sh` built the checked-in archives beside it, which hold a `src/com/facebook/buck/Main.java` path. No test names that path.
- In a Rust file, `.buckconfig` outside a string is a field access. The first run of `rename_runtime.py` renamed `self.buckconfig` in `app/buck2_interpreter_for_build/src/interpreter/buckconfig.rs`, and the build failed with E0609. The rules for file names now apply only to strings and comments in Rust files, and only the rules that rename constants apply to code.
- The selective debugging test binary `prelude/apple/tools/selective_debugging/test_resources/HelloWorld` names its object files by `buck-out/` paths in N_OSO symbol strings, and `scrubber.py` recognizes build outputs by that prefix. `manual_edits.py` rewrites the four strings to `yak-out/` and pads each with one NUL byte, which keeps every string at its offset in the string table. `python3 -m unittest apple.tools.selective_debugging.scrubber_test apple.tools.selective_debugging.spec_test`, run from `prelude/`, passes its 14 tests.
- Three more binary files hold the old names, which the text rules skip. The event logs `tests/core/console/fixtures/my_genrule0.proto` and `my_genrule1.proto` recorded builds under `buck-out` and `~/.buck/buckd`. `prelude/toolchains/android/test/com/facebook/buck/util/zip/sample-bytes.dat` holds `buck-out` in the bytes that `ZipOutputStreamTest` compresses.
- Upstream named the build files of its test data `TARGETS.fixture` (633 files), `BUCK.fixture` (6 files), and `TARGETS.test` (2 files). The root `.yakconfig` lists `tests` and `examples` under `[project] ignore`, so the repository's own build reads none of them under any name.
- `prelude//rust/rust-analyzer/resolve_deps.bxl` found the source folder of a crate that the host cannot build by removing a `/TARGETS` or `/BUCK` suffix from its build file path. For any other build file name, the file name stayed in the path. The path also joined the cell's name to the project root, so `lib/BUILD` in a root cell named `root` gave `<project root>/root/lib`.
- In a Rust file, the lexer of `rename_runtime.py` splits `section: "buck2"` into code and a string literal, so a pattern that includes the code around a literal matches no single part. `rename_env.py` runs such patterns over the whole text of each Rust file.
- `buck2_resource_control` names a configuration section and a crate, so the rule for section names skips `Cargo.toml` and `YAK` files.
- A comment in `app/buck2_server/src/ctx.rs` named the key `buck2.default_remote_execution_use_case`, but `DEFAULT_RE_USE_CASE_KEY` reads it from the `build` section.
- None of the four jars that the prelude downloads from the upstream release reads a `BUCK_` or `BUCK2_` variable. Their class files hold `BUCK_` only in the field names of `BuckConstant`, which no class calls.
- The command rule skips `buck2` after a `/`, which keeps crate paths such as `app/buck2` intact, but it also skipped paths to the binary. `test_is_buck2_exe` in `app/buck2_wrapper_common/src/is_buck2.rs` failed on `/dir/buck2`. `manual_edits.py` renames that path and the binary paths in `docs/developers/perf/scripts/bin_waste.py` and `examples/with_prelude/README.md`.

## Decision Log

- 2026-09-28: The tool is for internal use at the owner's company and is not published to crates.io, PyPI, or npm. Registry availability therefore does not rank the names.
- 2026-09-28: The build file is named `BUILD`, as the owner asked. A directory on a case-insensitive file system, such as the macOS and Windows defaults, cannot hold both a `BUILD` file and a `build/` directory, and Bazel's `BUILD` files in vendored code would be read as this tool's build files. The default list is therefore `BUILD.yak` then `BUILD`, so a directory can use `BUILD.yak` where `BUILD` cannot live. `find_buildfile` in `app/buck2_common/src/find_buildfile.rs` takes the first name in the list that exists. No directory in this repository that holds a `BUCK` file also holds an entry named `build`.
- 2026-09-28: The owner chose yak. It is three letters long, and it names the yak shaving that build systems are known for. Homebrew has no formula or cask named `yak`, and no Debian stable package installs a `yak` command. Rhino 3D ships a `yak` package manager, which is not a build tool.
- 2026-09-28: The build file is named `YAK`, which replaces the `BUILD` decision above at the owner's request. A `YAK` file can sit beside a `build/` directory on a case-insensitive file system, and vendored Bazel `BUILD` files stay out of the build. `DEFAULT_BUILDFILES` therefore holds only `YAK`, and a project that wants another name sets `[buildfile] name`. GitHub's Linguist highlights `BUCK` and `BUILD` as Starlark by file name but not `YAK`, so `.gitattributes` maps it.
- 2026-09-28: yak reads only the new names, as the owner chose. It keeps no fallback to `BUCK`, `BUCK.v2`, `.buckconfig`, `.buckroot`, `buck-out`, or `BUCK2_` variables, so each file and marker has one name. A project can keep `BUCK` files by setting `[buildfile] name = BUCK` in its `.yakconfig`.
- 2026-09-28: The Java packages move from `com.facebook.buck` to `dev.yak`, as the owner chose, so `com.facebook.buck.jvm.java` becomes `dev.yak.jvm.java`. The domain `yak.dev` is not the owner's, but the packages ship only inside the company's tool.
- 2026-09-28: The 97 `buck2*` Cargo packages and their directories take `yak` names, as the owner chose. Milestone 5 runs last, so the earlier milestones stay small enough to review.
- 2026-09-28: The imports of `com.facebook.infer.annotation` keep their package, because the Infer annotation jar defines it. The owner asked to remove the `com.facebook` code that only makes sense inside Facebook, and Infer is a public project.
- 2026-09-28: Test inputs that stand for user code take `com.example` names, because they name no package of this repository.
- 2026-09-28: `[buildfile] name` lists the exact build file names, and the `name_v2` key and the `.v2` variant of each name are removed. They let Buck1 and Buck2 build files share a directory, and yak reads no Buck1 files.
- 2026-09-28: The test data build files `TARGETS.fixture`, `BUCK.fixture`, and `TARGETS.test` become `YAK.fixture` and `YAK.test`, so the test data keep one fixture name.
- 2026-09-28: The daemon directory and its files take `yakd` in place of `buckd`, and the process title `buck2d[<project>]` becomes `yakd[<project>]`.
- 2026-09-28: Shell completion no longer registers a `buck` command, because that name belonged to Meta's wrapper. `test_build_flags_buck_bin` in `tests/core/completion/test_completion.py`, which ran once per shell, is removed with it.
- 2026-09-28: The reserved directory in `yak-out` is `._yak`, which replaces `._buck2`.
- 2026-09-28: Milestones 1 and 2 rename only the names a user sees. Rust and Starlark identifiers that contain `buck`, such as `BuckConfig`, `RESERVED_BUCK_OUT_PREFIX`, and the `buck2` and `buck2_client` attributes of `yak_bundle` in `defs.bzl`, keep their names. Milestone 5 renames the two attributes together with the crate targets they name.
- 2026-09-28: The event logs under `tests/core/console/fixtures/` and `sample-bytes.dat` keep the old names. A string of a different length breaks the protobuf framing of an event log, and the console tests check only the action digest and the console output. `ZipOutputStreamTest` treats `sample-bytes.dat` as arbitrary bytes.
- 2026-09-28: `resolve_deps.bxl` takes the directory of the build file's project-relative path as the source folder. `[buildfile] name` can give the build file any name, and a cell's directory can have a name other than the cell's.
- 2026-09-28: `BUCK2_` and `BUCK_` variables take the `YAK_` prefix, and `BUCKD_` variables take `YAKD_`. `BUCK2_NO_BUCKD` and `BUCK2_TEST_FAIL_BUCKD_AUTH` become `YAK_NO_YAKD` and `YAK_TEST_FAIL_YAKD_AUTH`. No two old names map to one new name.
- 2026-09-28: The variables that yak and the prelude set for actions and tools, such as `BUCK_SCRATCH_PATH` and `BUCK_BUILD_ID`, take the `YAK_` prefix too. The prelude's tools and the Java sources read the new names, and the downloaded jars read none of them.
- 2026-09-28: Constants, enum values, and placeholders whose names contain `BUCK`, such as `BUCK_WRAPPER_UUID_ENV_VAR` and `BUCKD_INFO_MISSING`, keep their names, as other identifiers do. `KEEP` in `rename_env.py` lists them.
- 2026-09-28: The configuration sections `[buck2]`, `[buck2_re_client]`, `[buck2_resource_control]`, `[buck2_system_warning]`, `[buck2_hydration]`, and `[buck2_metadata]` take the `yak` prefix.
- 2026-09-28: The `--no-buckd` flag is `--no-yakd`. The `no_buckd` field keeps its name, and its `clap` attribute names the flag.
- 2026-09-28: `host_info()` loses its `buck2` field. The field was always true, and it told Buck2 from Buck1 in rules that both tools loaded.
- 2026-09-28: The scripts of each milestone live beside this plan in `docs/exec-plans/active/2026-09-28-rename-the-fork/`. `rename_runtime.py` skips `docs/exec-plans/`, so it never rewrites its own patterns.

## Outcomes & Retrospective

Milestones 1 and 2 landed on 2026-09-28. The binary is `yak`, it reads `YAK` and `.yakconfig` files, and it writes `yak-out`. The two milestones landed as one commit, because the integration tests and the repository's own build need the binary and the files it reads to change together. The environment variables, configuration sections, Java packages, crates, and prose remain. For the scripts of later milestones, a rule for Rust files needs a lexer, because the same word is a file name inside a string and a field name outside one. A pattern that ends at `$` needs multiline mode to match at the end of each line. Binary fixtures, test inputs that type part of a renamed name, and paths that end in the binary's name need edits of their own.

Milestone 3 landed on 2026-09-28. The variables, the configuration sections, and the hidden daemon flag take yak names. Output that sorts or cuts names changes when the names change, so golden files and a test that assumed an order needed updates. A pattern that includes the code around a Rust string literal needs the whole text of the file, because the lexer separates the literal from its code.

## Context and Orientation

The names the binary uses are defined in these files:

| Name | Where it is defined |
| --- | --- |
| Binary `yak` | `[[bin]]` in `app/buck2/Cargo.toml` and the clap `name` in `app/buck2/src/lib.rs` |
| Build file `YAK` | `DEFAULT_BUILDFILES` in `app/buck2_common/src/buildfiles.rs` |
| `.yakconfig`, `.yakconfig.local`, `.yakconfig.d/`, `/etc/yakconfig`, and the Windows `yakconfig.d` | `app/buck2_common/src/legacy_configs/path.rs` |
| Project root markers `.yakconfig` and `.yakroot`, and the `~/.yak` directory | `app/buck2_common/src/invocation_roots.rs` |
| Output directory `yak-out` and its reserved directory `._yak` | `app/buck2_common/src/invocation_paths.rs`, and `app/buck2_common/src/ignores/ignore_set.rs` for the ignore rule |
| `.yaksettings.toml` | `app/buck2_common/src/settings/parser.rs` |
| Files that `init` writes | `app/buck2_client/src/commands/init.rs` |
| Environment variables | each `buck2_env!` call, with the `YAK_` prefix |

The repository's own Buck build is described in `ARCHITECTURE.md` under "Two build definitions". Most Java and Kotlin sources of the JVM and Android toolchain live under `prelude/toolchains/android/src/com/facebook/` and `prelude/toolchains/android/test/com/facebook/`, and Surprises & Discoveries lists the other three trees. `prelude/toolchains/java.bzl`, `prelude/toolchains/kotlin.bzl`, and `prelude/toolchains/android.bzl` name targets in that tree. The tech-debt tracker lists the four jars that the prelude downloads from an upstream release under "Downloads from upstream releases".

Counts on 2026-09-28, before milestone 1, from the repository root:

| Item | Count |
| --- | --- |
| Matches of `buck2`, any case | 32529 in 2674 files |
| `BUCK` files | 577, of which 191 are in `examples/` |
| Mentions of the `BUCK` file name | 586 in 182 files |
| Matches of `.buckconfig` | 346 in 118 files |
| Matches of `buck-out` | 940 in 323 files |
| Matches of `BUCK2_` variables | 886 in 615 files |
| Matches of `com.facebook` | 3847 in 1014 files |
| Cargo packages | 143, of which 97 are named `buck2*` |

`git grep -c 'com.facebook' -- . ':!docs/exec-plans/'` gives the `com.facebook` row.

## Plan of Work

A script in `docs/exec-plans/active/2026-09-28-rename-the-fork/` performs each mechanical rename, so a milestone can be rerun from a clean tree and reviewed as the script plus its output. Milestones 1 and 2 use two scripts. `rename_runtime.py` applies pattern rules and renames files, and `manual_edits.py` applies the edits that each need a decision and checks how many times each edit matches.

1. Milestone 1. Change the names in the files listed under Context and Orientation. Set `DEFAULT_BUILDFILES` to `YAK`, the configuration files to `.yakconfig`, `.yakconfig.local`, `.yakconfig.d/`, `/etc/yakconfig`, and `/etc/yakconfig.d/`, the root marker to `.yakroot`, the home directory to `~/.yak`, the daemon directory to `~/.yak/yakd`, the output directory to `yak-out` and its reserved directory to `._yak`, and the settings file to `.yaksettings.toml`. Remove the `name_v2` key and the `.v2` variants of `[buildfile] name`. Update the unit tests and the golden files that print these names.
2. Milestone 2. Rename the 577 `BUCK` files to `YAK` and `.buckconfig` to `.yakconfig` in the repository and the example projects, add `YAK linguist-language=Starlark` to `.gitattributes`, map `YAK` to Starlark in `.vscode/` settings, and update `.github/` and the test harness in `tests/e2e_util/`. The test data files `TARGETS.fixture`, `BUCK.fixture`, and `TARGETS.test` become `YAK.fixture` and `YAK.test`, and their `.yakconfig` files select the new names.
3. Milestone 3. Rename each `BUCK2_` and `BUCK_` variable to the `YAK_` prefix in its `buck2_env!` call and in every reader, including `tests/`, `.github/`, and the documentation. Rename the `[buck2]` configuration sections and the sections that start with `buck2_`, the `--no-buckd` flag, `_BUCK_COMPLETE_BIN`, and the `buck2` field of `host_info()`.
4. Milestone 4. Move the five `com/facebook` trees to the path of the chosen package root, and update package declarations, imports, `META-INF/services` files, and the labels in `.bzl` and `YAK` files. Change the `java_package` option in the eight `.proto` files, the two JUnit runner system properties, and the three annotation processor options, together with their readers. Leave the `com.facebook.infer.annotation` imports as they are. Leave the archives under `util/unarchive/testdata/` and their generator unchanged, or regenerate the archives with the script. Build the four bootstrap jars from the moved sources, store them where the company hosts build artifacts, and point the prelude's downloads at them.
5. Milestone 5. Rename the 97 `buck2*` Cargo packages and their directories, with the matching `YAK` targets and `use` paths, the `#[buck2(...)]` attribute of the error derive macro, and the `buck2` and `buck2_client` attributes of `yak_bundle` in `defs.bzl`.
6. Milestone 6. Update `website/docs/`, `docs/developers/`, `README.md`, `AGENTS.md`, `ARCHITECTURE.md`, and `CHANGELOG.md`, including the name Buck2 in prose, Buck1 commands such as `buck install`, the word buckconfig, the skill directory `.claude/skills/buck2-rule-basics/`, the website's `projectName`, and the publisher of the VS Code extension in `starlark-rust/vscode/package.json`. The links to the fork's GitHub repository change when the repository is renamed.

## Validation and Acceptance

Commands run from the repository root unless a step names another directory.

- `cargo build --bin=yak` and `python3 test.py` pass after each milestone.
- The integration tests pass against the new binary, with golden files regenerated and reviewed (`tests/README.md`).
- `target/debug/yak init` in an empty directory writes `.yakconfig` and no `.buckconfig`.
- `target/debug/yak build //:yak` builds the binary with the repository's own `YAK` files after `reindeer buckify`.
- In a project that vendors the prelude, `yak build prelude//toolchains/android/...` fails only in the three targets the tech-debt tracker lists under "The JVM toolchain has targets that do not build".
- The JUnit reports of `yak test prelude//toolchains/android/test/...` list the same failures before and after milestone 4.
- `git grep -n -E '(^|[^.[:alnum:]_])com[./]facebook' -- . ':!docs/exec-plans/' | grep -v 'com\.facebook\.infer\.annotation'` prints nothing.

## Idempotence and Recovery

Each milestone's script runs on a clean tree and can be rerun after `git checkout` of the files it changed. To rerun milestones 1 and 2, copy `rename_runtime.py` and `manual_edits.py` out of the checkout, check out `b25e8f97d8`, and run `rename_runtime.py --apply` and then `manual_edits.py` from the repository root. `manual_edits.py` exits with a list of failures when an edit matches a different number of times than it expects. To rerun milestone 3, copy `rename_runtime.py`, `rename_env.py`, and `manual_edits_env.py` out of the checkout together, check out `04fcc47b52`, and run `rename_env.py --apply` and then `manual_edits_env.py`. Moving the Java sources with `git mv` keeps their history. The bootstrap jars are stored before the prelude points at them, so a failed upload leaves the upstream downloads in place.
