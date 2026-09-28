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
- [ ] Milestone 1: the binary uses the new names for its files and directories.
- [ ] Milestone 2: the repository's own build files and the example projects use `YAK` and `.yakconfig`.
- [ ] Milestone 3: the environment variables take the `YAK_` prefix.
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
- `buck2 test` passes a JVM test target whatever its JUnit results (tech-debt tracker, "`buck2 test` reports failing JVM tests as passing"). The JUnit reports are the evidence for milestone 4.
- The ABI tests compare compiler diagnostics that print a caret under the error column, so a package name of a different length moves the caret. `JavaInMemoryFileObjectTest` counts the directory entries of a jar, so a path of a different depth changes the count.
- `prelude/toolchains/android/test/com/facebook/buck/util/unarchive/testdata/generate_archives.sh` built the checked-in archives beside it, which hold a `src/com/facebook/buck/Main.java` path. No test names that path.

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

## Outcomes & Retrospective

Not started.

## Context and Orientation

The names the binary uses are defined in these files:

| Name | Where it is defined |
| --- | --- |
| Binary `buck2` | `[[bin]]` in `app/buck2/Cargo.toml` and the clap `name` in `app/buck2/src/lib.rs` |
| Build files `BUCK.v2` and `BUCK` | `DEFAULT_BUILDFILES` in `app/buck2_common/src/buildfiles.rs` |
| `.buckconfig`, `.buckconfig.local`, `.buckconfig.d/`, `/etc/buckconfig`, and the Windows `buckconfig.d` | `app/buck2_common/src/legacy_configs/path.rs` |
| Project root markers `.buckconfig` and `.buckroot`, and the `~/.buck` directory | `app/buck2_common/src/invocation_roots.rs` |
| Output directory `buck-out` | `app/buck2_common/src/invocation_paths.rs`, and `app/buck2_common/src/ignores/ignore_set.rs` for the ignore rule |
| `.bucksettings.toml` | `app/buck2_common/src/settings/parser.rs` |
| Files that `init` writes | `app/buck2_client/src/commands/init.rs` |
| Environment variables | each `buck2_env!` call, 110 distinct `BUCK2_` names |

The repository's own Buck build is described in `ARCHITECTURE.md` under "Two build definitions". Most Java and Kotlin sources of the JVM and Android toolchain live under `prelude/toolchains/android/src/com/facebook/` and `prelude/toolchains/android/test/com/facebook/`, and Surprises & Discoveries lists the other three trees. `prelude/toolchains/java.bzl`, `prelude/toolchains/kotlin.bzl`, and `prelude/toolchains/android.bzl` name targets in that tree. The tech-debt tracker lists the four jars that the prelude downloads from an upstream release under "Downloads from upstream releases".

Counts on 2026-09-28, from the repository root:

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

A script in the repository performs each mechanical rename, so a milestone can be rerun from a clean tree and reviewed as the script plus its output.

1. Milestone 1. Change the names in the files listed under Context and Orientation. Set `DEFAULT_BUILDFILES` to `YAK`, the configuration files to `.yakconfig`, `.yakconfig.local`, `.yakconfig.d/`, `/etc/yakconfig`, and `/etc/yakconfig.d/`, the root marker to `.yakroot`, the home directory to `~/.yak`, the output directory to `yak-out`, and the settings file to `.yaksettings.toml`. Update the unit tests and the golden files that print these names.
2. Milestone 2. Rename the 577 `BUCK` files to `YAK` and `.buckconfig` to `.yakconfig` in the repository and the example projects, add `YAK linguist-language=Starlark` to `.gitattributes`, map `YAK` to Starlark in `.vscode/` settings, and update `.github/` and the test harness in `tests/e2e_util/`. The integration test data keep `TARGETS.fixture`, which their `.buckconfig` files select, so only those configuration files change name.
3. Milestone 3. Rename each `BUCK2_` variable to the `YAK_` prefix in its `buck2_env!` call and in every reader, including `tests/`, `.github/`, and the documentation.
4. Milestone 4. Move the five `com/facebook` trees to the path of the chosen package root, and update package declarations, imports, `META-INF/services` files, and the labels in `.bzl` and `BUCK` files. Change the `java_package` option in the eight `.proto` files, the two JUnit runner system properties, and the three annotation processor options, together with their readers. Leave the `com.facebook.infer.annotation` imports as they are. Leave the archives under `util/unarchive/testdata/` and their generator unchanged, or regenerate the archives with the script. Build the four bootstrap jars from the moved sources, store them where the company hosts build artifacts, and point the prelude's downloads at them.
5. Milestone 5. Rename the 97 `buck2*` Cargo packages and their directories, with the matching `BUCK` targets and `use` paths.
6. Milestone 6. Update `website/docs/`, `docs/developers/`, `README.md`, `AGENTS.md`, `ARCHITECTURE.md`, and `CHANGELOG.md`.

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

Each milestone's script runs on a clean tree and can be rerun after `git checkout` of the files it changed. Moving the Java sources with `git mv` keeps their history. The bootstrap jars are stored before the prelude points at them, so a failed upload leaves the upstream downloads in place.
