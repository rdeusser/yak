# Remove Meta-internal code, configuration, and analytics

Follows [the plan contract](../../PLANS.md).

## Purpose

The repository builds, tests, and documents an open-source build system that depends on nothing inside Meta.
No code path collects analytics or sends data to a Meta service.
Code, build files, configuration, tests, documentation, and the website name no Meta cells, teams, internal URLs, internal services, or internal tools.
The copyright headers and license files stay, because the MIT and Apache-2.0 licenses require them.
A shortlist of bird-themed names prepares the rename, which waits for the owner's choice.

The owner's criteria (2026-09-27):

- Remove Facebook-specific internal things, for example `fbsource`, `internalfb`, `is_full_meta_repo`, `fbcode`, and `com.facebook`.
- Remove the Meta test harness and the `com.facebook` namespace where they only make sense inside Meta.
- Remove all analytics.
- Propose bird-themed names, excluding Falcon.
- Leave as little connection to Facebook as possible.

## Progress

- [x] Inventory the Rust services and analytics, the prelude, the test harness, the Buck self-build, and the documentation and metadata (2026-09-27).
- [x] Milestone 1, first part: remove every `fbcode_build` branch, the `buck2_eden` crate, the modules only internal builds declared, and `cfg(fbcode_build)` from the Cargo check-cfg lists. `cargo build --bin=buck2` passes.
- [x] Remove the autocargo headers, the Meta `authors` entries, and the docs.rs `documentation` links from Cargo manifests, and point `repository` at the fork.
- [x] Milestone 1, remaining: `is_open_source()`, `if_else_opensource!`, `facebook_only()`, and the `@oss-disable` and `@oss-enable` markers in Rust are gone (2026-09-28).
- [x] Milestone 2: remove analytics and Meta service integrations (2026-09-28).
  - The crates `buck2_cmd_rage_client`, `buck2_explain`, `buck2_health_check`, `buck2_health_check_proto`, and `scribe_client` are deleted.
  - The Manifold, Scribe, Scuba, x2p, VPNless, `CertState`, tenting, Eden, and `fbinit` code is deleted, and removed `ErrorTag` values are reserved.
  - `buck2 init` writes only the `root`, `prelude`, and `toolchains` cells and the `config` alias.
  - On Linux, `cargo build --bin=buck2`, `cargo fmt --all -- --check`, and `python3 test.py` pass (2026-09-28). `test_perf_thread_instruction_counter` in `app/buck2_util` needs `perf_event_open` access, and it skips on GitHub Actions.
- [x] Remove the Meta Remote Execution extensions that the Bazel API cannot carry (gang workers, RE dependencies, the dynamic image), the Meta device-lab configuration in `android_instrumentation_test`, and the tests that exercised them (2026-09-28). `buf breaking` passes for the changed protos, and the snapshot binary loads and analyzes the changed prelude rules. The Linux build and `python3 test.py` cover the Rust changes.
- [x] Remove the prelude metadata that only Meta's Tpx test runner reads (`tpx:*` labels, `run_as_bundle`, `TPX_LIST_TESTS_COMMAND`, `TestListingInfo`, and the Go test lister) (2026-09-28). Load, typecheck, and provider checks with the snapshot binary match before and after, apart from the intended `go_test` environment change.
- [x] Milestone 3: remove Meta-only code from the prelude.
  - `prelude/` references none of the `fbcode`, `fbsource`, `fbcode_macros`, `buck`, or `ovr_config` cells.
  - It has no `is_full_meta_repo`, `meta_only`, export markers, or `oncall` calls.
  - It drops the Kosabi plugins, the JVM abtesting and analytics code, the Apple bundle telemetry logger, and the gopackagesdriver telemetry interface.
  - All 624 `.bzl` and `.bxl` modules evaluate in a project without the Meta aliases.
  - Nine example projects give the same `targets` and `cquery` results as before.
  - 136 Python unit tests and the gopackagesdriver Go tests pass.
  - The deletion of `prelude/python/tools/gen_bytecode_bundle.py` (an unused Meta tool) waits for the owner.
- [x] Milestone 4: replace `shim/` and the Meta cell names in the Buck build (2026-09-28).
  - `build_defs/`, `toolchains/`, and `third-party/` replace `shim/`, and the `BUCK` files use `//app/...`, `//third-party/rust:...`, and `prelude//os:...` labels.
  - `.buckconfig.d/common.buckconfig`, the `[oss]` configuration, `lint_levels.bzl`, the `clippy_config` target, and the `oncall` calls are gone.
  - `remote_execution/oss/re_grpc` moved to `remote_execution/re_grpc`.
  - `bootstrap/buck2`, `bootstrap/rust-project`, and their Windows shims are deleted (see Decision Log).
  - `third-party/rust/fixups/object` is deleted. Removing `backtrace` from `third-party/rust/Cargo.toml` removed the last `object` version with a build script, and `reindeer buckify` fails on a build-script fixup for a crate without one.
  - The 18 fixup directories for crates outside the generated `third-party/rust/Cargo.lock` are deleted (`argmin-math`, `aws-lc-fips-sys`, `bzip2-sys`, `cmake-rs`, `icu_normalizer_data`, `icu_properties_data`, `io-lifetimes`, `jemalloc-sys`, `lalrpop`, `lexical-core`, `pest`, `proc-macro-error`, `proc-macro-error-attr`, `proc-macro-hack`, `radium`, `reqwest`, `target-triple`, and `winreg`).
  - `third-party/rust/fixups/zip` sets `CARGO_PKG_VERSION`, which `//tools/starlark_fmt:starlark_fmt_lib` needs through `ruff_db`.
  - `third-party/rust/fixups/portable-atomic` sets `CARGO_PKG_NAME`, which the crate's build script reads.
  - `third-party/rust/fixups/ruff_python_formatter` sets `CARGO_PKG_AUTHORS`, `CARGO_PKG_DESCRIPTION`, and `CARGO_PKG_VERSION`, which the crate's clap derive reads at compile time.
  - `third-party/rust/Cargo.toml` builds `libfuzzer-sys` without its `link_libfuzzer` feature, because `//starlark-rust/starlark/fuzz:starlark-fuzz` only checks that the fuzz target compiles.
  - `//dice/dice_examples:memory_by_key` is removed (see Decision Log).
  - On Linux, `./bootstrap/reindeer --third-party-dir third-party/rust buckify`, `target/debug/buck2 build //:buck2`, `target/debug/buck2 build //app_dep_graph_rules:test_buck2_dep_graph`, and `target/debug/buck2 targets //...` succeed (2026-09-28). The macOS build is unverified.
  - On Linux, `buck2 build` of the 265 targets outside `third-party/rust/` fails only in `//shed/completion_verify/packages:zsh` and `//shed/completion_verify/packages:fish`, which run `dnf` (2026-09-28).
- [x] Milestone 5: port the integration tests to pytest and delete the Meta test harness (2026-09-28).
  - `tests/conftest.py`, `tests/requirements.txt`, and `tests/README.md` run the suite with plain pytest, and `.github/workflows/integration-tests.yml` runs it on Linux.
  - Tests that need a Remote Execution backend, cgroup delegation, or helper binaries carry markers and skip without them.
  - Golden files and event log fixtures are regenerated with a binary built from this repository.
  - A whole-suite run on Linux gave 1717 passed, 12 failed, and 233 skipped.
  - Tests of removed behavior are deleted (the action host name, the VS Code client id, and the action digest trace).
  - The `whatup` and peak-memory goldens are regenerated, and `test_dep_files_ignore_missing_digests` is marked `remote_execution`.
  - The other failures need `ps`, `lldb`, a user other than root, a cgroup below the root of the cgroup namespace, or Python 3.12, and `tests/README.md` lists them.
  - The files changed after the whole-suite run give 125 passed and 15 skipped.
  - A final whole-suite run on Linux under Python 3.12, as a user other than root and with `ps` and `lldb` installed, gave 1726 passed, 230 skipped, and 3 expected failures (2026-09-28).
  - `.github/workflows/integration-tests.yml` installs `lldb` and sets `kernel.yama.ptrace_scope` to 0 for `test_thread_dump`.
  - The CI setup actions install Go 1.26 (see Decision Log). With Go 1.26, `clang`, and `lld`, the 30 tests in `tests/prelude/test_prelude_rules.py` pass on Linux, and all 19 Go tests among them fail with Go 1.22.
- [x] Milestone 6, examples: delete the 196 `oncall` calls and `examples/remote_execution/internal/`, rename the demo app to `com.example.demoapp`, and fix `examples/persistent_worker/BUCK`, which loaded `@fbcode_macros`. `buck2 targets //...` gives the same result in every example project before and after, except that `persistent_worker` now loads.
- [x] Milestone 6, documentation and metadata:
  - The site uses `@docusaurus/preset-classic` without Google Analytics, Algolia, or the internal-content tags.
  - The site pages and `docs/developers/` no longer describe Meta services or link inside Meta.
  - The root files point at the fork, and `SECURITY.md` is new.
  - The workflows drop the owner checks, the NativeLink job, and the prelude-hash job.
  - `.claude/settings.json` (an `arc f` Stop hook) is deleted.
  - The lockfiles were pruned by script without npm or yarn, so the Docusaurus build is unverified.
- [x] Milestone 6, remaining: `buck2.build` links in Rust and the prelude point at the fork's site (2026-09-28). The links in `shim/` go with milestone 4.
- [x] Milestone 7: shortlist names (see Decision Log).
- [ ] Milestone 8: drop the `none` cell aliases, sweep for remaining references, and validate the whole workspace.
  - The 11 example `.buckconfig` files no longer declare the `none` cell or the Meta cell aliases (2026-09-28). The load check ran `buck2 targets //...` and `buck2 cquery //...` in each example project with the working-tree prelude vendored into it:
    - `hello_world`, `bootstrap`, `toolchains/cxx_zig_toolchain`, `toolchains/go_toolchain`, and `toolchains/python_toolchain` give the same targets as before.
    - `android/demoapp` gives the same targets apart from the `com.example.demoapp` package.
    - `persistent_worker` (25 targets) and `toolchains/conan_toolchain` (11 targets) load.
    - `no_prelude` and the four `remote_execution` projects, which declare no prelude cell, load without one.
    - `bxl_tutorial`, `with_prelude`, and the `android/demoapp` `cquery` fail as they did at `903bfd7a61`, and the tech-debt tracker records them.
    - `vscode` fails `cquery` as it did at `903bfd7a61`, because it registers execution platforms only on Windows and Linux hosts or with Remote Execution enabled.
  - `# pyre-strict`, Pyre suppressions, `@nolint`, `@noautodeps`, and Meta `@lint-ignore` pragmas are removed from 92 files outside `tests/` and `shim/` and from 300 more in `tests/`.
  - The `fb_build_info` ELF section is `build_info`, and `BUCK2_TEST_TPX_USE_TCP` is `BUCK2_TEST_EXECUTOR_USE_TCP`.
  - psutil builds with only its `network` feature, which drops `darwin-libproc` and its `github.com/fbsource` patch from `Cargo.toml` and `Cargo.lock`. `cargo tree --locked` resolves, and the Linux and macOS builds compile it.
  - `CHANGELOG.md` lists the user-visible removals and renames.
  - `test.py` checks the Git working tree, and its `--git` flag and `hg` commands are gone.
  - The fbpython shebangs in `prelude/apple/tools/` and the Meta names in test data (`xplat`, `lionhead`, the `buck` cell) are replaced.
  - A sweep of tracked and untracked files for `fbcode`, `fbsource`, `internalfb`, `is_full_meta_repo`, `oss-disable`, `oss-enable`, Scuba, Scribe, and Manifold finds them only in `docs/exec-plans/`, `CHANGELOG.md`, and the porting guidance in `AGENTS.md` and `docs/developers/basics.md` (2026-09-28). `oncall(` remains only in the generic `oncall` function, its tests, and test inputs. `com.facebook` remains in the packages the Decision Log keeps.
  - A second sweep looked for Meta app names, internal repository paths, task and diff IDs, and Meta hosts (2026-09-28). It found no task or diff IDs.
    - The examples in `prelude/decls/` and `website/docs/rule_authors/custom_macros.md` use neutral names in place of Messenger, `fb4a`, `com.facebook.orca`, and `//java/com/facebook/...` targets.
    - Comments no longer cite a Reality Labs script path or Buck1 Java source, and the `IgnoreSet` doc describes its matching without Buck1 classes.
    - The kapt and KSP steps no longer single out a Kotlin compiler plugin named `di.jar`. That rule served Meta's dependency injection plugin and read its `com.facebook.kotlin.di:kspActive` option. Kapt receives only the all-open plugin, and standalone KSP receives only symbol-processing plugins. Two tests in `DaemonKotlincToJarStepFactoryTest` cover the lists, and the kapt test fails when the `di.jar` rule is restored.
    - `JavacVersion.java` is deleted. No target built it, and it imported a Buck1 class that the repository lacks.
    - The aapt test data under `prelude/toolchains/android/test/com/facebook/buck/android/aapt/testdata/` is deleted, because no test or rule read it.
    - The `CrashAnalyzer` examples and the `CrashAnalyzerTest` inputs use `com.example` names in place of Meta apps and libraries (Oculus, QPL, `libfacebook.so`).
    - In a project that vendors the prelude, `buck2 build` of the changed Kotlin, Java, and test runner packages succeeds. The JUnit reports of the Kotlin, test runner, and aapt test targets show 379 passes and no failures when the test JVM runs with `-Dnet.bytebuddy.experimental=true` (tech-debt tracker, "JVM tests fail in their JUnit reports").
  - The daemon no longer reads `~/.buckconfig.d/experiments_from_buck_start`, which Meta's `buck_start` wrapper wrote with Gatekeeper experiments (2026-09-28). `const_format`, which only that code used, is gone from the manifests and `Cargo.lock`.
  - The unit tests of `buck2_client` and `buck2_client_ctx` no longer use the `fb//` cell, the `metaguest` home directory, the header of Meta's generated mode files, or a Tupperware cgroup path.
  - On Linux, `cargo build --bin=buck2`, `cargo fmt --check`, and `python3 test.py buck2_client buck2_client_ctx buck2_server_ctx buck2_wrapper_common` pass. The integration tests give 1726 passed, 230 skipped, and 3 expected failures, as before.
  - On macOS on 2026-09-28, at `04fcc47b52`, which renamed the binary to `yak`, `cargo build --bin=yak` passes, and `python3 test.py` passes clippy and rustdoc. The unit and doc tests pass apart from the paging tests of `starlark`, which fail when they run in parallel (tech-debt tracker, "The paging tests of `starlark` fail when they run in parallel").
  - Remaining: the integration tests and the Buck build on macOS.
  - Remaining: the `FBBuck2` key in the `Info.plist` of app bundles. `prelude/apple/apple_info_plist.bzl` adds it to each top-level `.app` bundle when `info_plist_identify_build_system` is true. The `apple_bundle` macro takes the default from `[apple] info_plist_identify_build_system`, which is true when unset, so every app bundle carries the key.
  - Remaining: `prelude/ide_integrations/visual_studio/msvs/absolutize_path.exe`, a 2.6 MB Windows binary with no source in this repository. It turns relative paths in compiler diagnostics into absolute paths, and its help text describes its `LOCAL_ROOT` argument as the path of an fbsource checkout. `gen_mode_configs.bxl` passes it to the generated Visual Studio projects as `AbsolutizePathExe`.
  - Remaining: 28 `ast-grep-ignore` markers in 17 files, such as `ast-grep-ignore: rust/buck2-no-std-hashmap`. They name rules of Meta's ast-grep configuration, which this repository lacks.
  - Remaining: the hidden `--skip-targets-with-duplicate-names` flag of the commands that load build files. Its doc comment in `app/buck2_client_ctx/src/common.rs` calls it "a hack for TD" and says not to use it.

## Surprises & Discoveries

- In `starlark-rust`, the `fbcode_build` gates chose between the in-tree `pagable` API and an older published `pagable`. The workspace depends on the in-tree crate through a path dependency (`Cargo.toml` `pagable = { version = "0.4.2", path = "pagable" }`), and `pagable/src/traits.rs` defines `take_arc_key`. Evaluating the gates with `fbcode_build = false` would have kept stubs that disable lazy heap binding, so those three files keep the internal branch.
- `buck2 rage` runs `hg snapshot create` and pipes its report (command line, working directory, host name, `git status`) to a `pastry` binary unless `--no-paste` is given (`app/buck2_cmd_rage_client/src/rage.rs`, `source_control.rs`). No other path in the Cargo build sends data to Meta: the Scribe sink factory returns `None`, the Manifold upload URL is `None`, and `should_upload_log()` returns false.
- `buck2 build //...` fails at `903bfd7a61`. `tools/starlark_fmt/BUCK` loads `@fbsource//tools/build_defs:cram_test.bzl` and `shed/completion_verify/packages/BUCK` loads `@fbcode//buck2/shed/rpm_download:packages.bzl`, and neither exists in `shim/`.
- `prelude/platforms/apple/build_mode.bzl` and `prelude/platforms/apple/platforms_map.bzl` do not parse. Each opens a `load` of a `meta_only` module whose closing parenthesis sits in an `@oss-disable` comment.
- The integration tests never shipped their `PACKAGE` fixtures (`git log --all -- ':(glob)tests/**/PACKAGE'` is empty), so the tests that need a root `PACKAGE` file cannot pass as exported.
- The built-in test orchestrator reads only the generic `static-listing` label (`app/buck2_test/src/orchestrator.rs`). Every `tpx:*` label and `TPX_LIST_TESTS_COMMAND` in the prelude served only Tpx.
- Parts of the prelude's test support only work under Tpx. `apple_test` returns the command `false` and passes its inputs through environment variables. `BaseRunner.runAndExit` in the JUnit runner exits 0 whatever the test outcome, so the built-in runner reports failing JVM tests as passing. The tech-debt tracker records both.
- `darwin-libproc`, which `Cargo.toml` patched to a fork under `github.com/fbsource`, came in only through psutil's `process` feature. `app/buck2_server` uses only `psutil::network`.
- A `cxx_toolchain` with `split_debug_mode = "split"` and clang passes `--fbcc-create-external-debug-info`, a flag only Meta's fbcc wrapper accepts. The tech-debt tracker records it.
- Inside Meta, `get_default_executor_config` in `app/buck2_server/src/daemon/common.rs` ran projects without execution platforms through hybrid Remote Execution. `test_dep_files_ignore_missing_digests` depended on that, because its tombstoned digest applies only when the dep file comes from the CAS. Run locally, it failed with `Misconfigured test, BUCK2_TEST_TOMBSTONED_DIGESTS to e6f56e7fc212856318bca48f00b119ed48e8f22b:78`.
- The release that `bootstrap/buck2` downloaded (2026-09-15) could not load the repository's Buck build. Its bundled prelude loads through the `fbsource` alias in `prelude/js/js.bzl`, and the working-tree prelude calls `dep_files_fingerprint_using_canonical_paths` (`prelude/java/javacd_jar_creator.bzl`), which that release lacks.
- Removing an `InstantEvent` variant breaks the replay of older event logs. `buck2 log replay` and `buck2 log whatup` fail with ``Missing `data` in `Instant` `` on a log that holds one of the removed events, so the checked-in log fixtures were recorded again.
- pytest sets `COLUMNS=80` in child processes while it captures output and leaves it unset under `-s`. clap wraps help text at `COLUMNS`, so the help goldens depended on the capture mode.
- The console prints network byte counts only from snapshots taken before a command ends. Recording `tests/core/console/fixtures/my_genrule0.proto` needed latency on the cache connection, which `tests/core/console/fixtures/README` describes.
- Meta's toolchain linked jemalloc into every binary. `app/buck2/bin/buck2.rs` leaves out `tikv_jemallocator` under `cfg(buck_build)`, and `third-party/rust/fixups/tikv-jemalloc-sys` does not build jemalloc, so a Buck-built `buck2` uses the system allocator. `//dice/dice_examples:memory_by_key` calls `mallctl` and failed to link with ``undefined reference to `mallctl'``.
- `third-party/rust/Cargo.lock` is generated on each `buckify` and ignored by Git, so crate versions in the Buck build follow crates.io. Removing `backtrace` changed which `object` versions `buckify` resolved, and the stale `object` fixup then failed `buckify` with `unused buildscript fixup for a package that has no build script`.
- `test.py` checked the working tree with `hg` unless `--git` was passed.
- `buck2 build --keep-going` builds none of the dependents of a failed target, so the `ruff_python_formatter` failure appeared only after the `portable-atomic` fixup.

## Decision Log

- 2026-09-27: Keep every `Copyright (c) Meta Platforms` and `Copyright (c) Facebook` header and both license files. Apache-2.0 section 4(c) requires retaining the copyright notices in the source of a derivative work, and the MIT license requires the notice in all copies.
- 2026-09-27: Remove code gated on `fbcode_build` with a script that evaluates each `cfg` with `fbcode_build = false`. Cargo never sets the cfg, so the removed code never compiled in this repository. The `starlark-rust` pagable files are the exception described under Surprises.
- 2026-09-27: Remove the Cargo `authors` fields. The copyright headers carry the attribution, and the field is optional.
- 2026-09-27: Keep the `com.facebook` Java and Kotlin packages of the JVM and Android toolchain until the rename picks a package name, because open-source JVM builds need that code and renaming twice doubles the churn. Delete the JVM code that only works inside Meta (the kosabi plugins and the abtesting and analytics scaffolding). The toolchain downloads three bootstrap jars (`jar_builder`, `zip_scrubber`, `cp_snapshot_generator`) from upstream GitHub releases with `com.facebook` main classes, so the package rename needs the fork to publish its own jars.
- 2026-09-27: Keep the `fbcode`, `fbsource`, `fbcode_macros`, `buck`, and `ovr_config` aliases in `buck2 init` and the example `.buckconfig` files until the bundled prelude stops naming those cells.
- 2026-09-27: Port `tests/core` and the isolated end-to-end tests to plain pytest, and delete the tests that need Meta infrastructure (EdenFS, Manifold, TPX, VPNless, in-repository builds inside Meta's monorepo).
- 2026-09-27: Keep the `bootstrap/` DotSlash files pointing at upstream GitHub releases until the fork publishes its own releases.
- 2026-09-28: Publish the documentation site as the GitHub Pages project site `https://rdeusser.github.io/buck2/`, and point `buck2.build` links there with the same paths. A custom domain later changes `url` and `baseUrl` in `website/config_impl.ts` and needs a `website/static/CNAME`.
- 2026-09-28: Send security reports through GitHub private vulnerability reporting (`SECURITY.md`) and conduct reports to the maintainers through GitHub, so the repository publishes no email address.
- 2026-09-28: Delete `.claude/settings.json`, whose only content was a Stop hook that ran Meta's `arc f` formatter.
- 2026-09-28: Remove the Meta cell aliases from `buck2 init`, because the prelude no longer names those cells. This replaces the 2026-09-27 decision to keep them.
- 2026-09-28: Remove the prelude labels and environment variables that only Tpx reads. Keep the Tpx result protocol in the JUnit and Erlang test runners until the `com.facebook` package rename, because the JVM toolchain does not compile in this repository today and the rename rewrites those files.
- 2026-09-28: Remove `remote_execution_gang_workers`, `remote_execution_dependencies`, and `remote_execution_dynamic_image`. The Bazel Remote Execution API has no field for them, so the executor dropped them before each request.
- 2026-09-28: Keep the `oncall()` build file function and the `oncall` target attribute, which work in open-source builds. Remove the calls that name Meta teams.
- 2026-09-28: Delete `bootstrap/buck2` and `bootstrap/buck2.exe`, which replaces the 2026-09-27 decision for those two files. The upstream release they download cannot load the repository's Buck build (see Surprises), so the Buck build runs `target/debug/buck2`. `bootstrap/reindeer` stays until the fork publishes releases.
- 2026-09-28: Delete `bootstrap/rust-project` and `bootstrap/rust-project.exe`. They downloaded `rust-project` from `facebook/buck2` releases, no file in the repository runs them, and `cargo build --bin rust-project` builds the tool from `integrations/rust-project/`.
- 2026-09-28: Set `COLUMNS=100` for the processes that the pytest harness starts (`tests/e2e_util/buck_workspace.py`), so help goldens match whether or not pytest captures output.
- 2026-09-28: Delete the integration tests of `ActionExecutionEnd` host names, the `vscode-fallback` client id, and the `ActionDigestTrace` event, because milestone 2 removed that behavior.
- 2026-09-28: Mark `test_dep_files_ignore_missing_digests` `remote_execution`, like `test_restart_cas_missing`, because both tombstone digests that only a download from the CAS reads.
- 2026-09-28: Require Python 3.12 for the integration tests, because `tests/core/external_cells/test_git.py` calls `shutil.rmtree(onexc=)`. `.github/workflows/integration-tests.yml` creates the virtual environment with `uv venv --python 3.12`.
- 2026-09-28: Make `test.py` check the Git working tree and remove its `--git` flag, because the repository uses Git and the `hg` default served Meta's repository.
- 2026-09-28: Drop the Buck target of `dice/dice_examples/bin/memory_by_key.rs`. The binary reads jemalloc statistics, and the Buck build links no jemalloc, so only Cargo builds it.
- 2026-09-28: Install Go 1.26 in the three CI setup actions, which installed Go 1.22. The prelude's Go tools call `os.CopyFS`, `slices.Collect`, and `maps.Keys`, which Go 1.23 added, and `prelude/go/package_builder.bzl` follows Go 1.26. The change is unverified on macOS and Windows.
- 2026-09-28: Name shortlist, ranked, for the project, the command, the build file, the configuration file, and the output directory. This ranking replaces the 2026-09-27 ranking, which put Gannet first.

  | Project | Command | Build file | Configuration | Output |
  | --- | --- | --- | --- | --- |
  | Shikra | `shikra` | `NEST` | `.shikraconfig` | `shikra-out` |
  | Gannet | `gannet` | `NEST` | `.gannetconfig` | `gannet-out` |
  | Hornero | `hornero` | `NEST` | `.horneroconfig` | `hornero-out` |
  | Goshawk | `goshawk` | `NEST` | `.goshawkconfig` | `goshawk-out` |
  | Harrier | `harrier` | `NEST` | `.harrierconfig` | `harrier-out` |
  | Osprey | `osprey` | `NEST` | `.ospreyconfig` | `osprey-out` |
  | Skua | `skua` | `NEST` | `.skuaconfig` | `skua-out` |

  The ranking comes from these lookups on 2026-09-28:

  | Name | crates.io | PyPI | npm | Homebrew | `.dev` domain | `.build` domain |
  | --- | --- | --- | --- | --- | --- | --- |
  | Shikra | free | free | free | free | no name servers | no name servers |
  | Gannet | free | taken | taken | free | has name servers | no name servers |
  | Hornero | free | taken | free | free | has name servers | no name servers |
  | Goshawk | taken | taken | taken | free | has name servers | no name servers |
  | Harrier | taken | taken | taken | free | has name servers | no name servers |
  | Osprey | taken | taken | taken | free | has name servers | has name servers |
  | Skua | taken | taken | taken | free | has name servers | no name servers |

  Shikra is a small hawk, so it keeps the bird-of-prey theme of the owner's Falcon example. A `.dev` domain without name servers is likely unregistered, which would give the Java packages a root such as `dev.shikra`. Other projects use the name Shikra (a multimodal language model, a USB hardware debugging tool, and a Qualcomm board codename that Ubuntu package names carry), and none of them is a build tool. The Gannet software found is a MATLAB toolkit for magnetic resonance spectroscopy. Neither name has a Debian source package. Trademarks are unchecked.

  Later on 2026-09-28, the owner chose `BUILD` as the build file name and said the tool is for internal use only, so [the rename plan](2026-09-28-rename-the-fork.md) replaces the `NEST` column and the registry lookups as ranking criteria. The owner then chose yak and changed the build file name to `YAK`, which the rename plan records.

  The name prefix also replaces `.buckconfig.local`, `.buckconfig.d/`, `.bucksettings.toml`, `.buckroot`, and `~/.buck/`. A migrating repository can accept both build file names with `[buildfile] name_v2 = NEST,BUCK` (the list parser in `app/buck2_common/src/legacy_configs/access.rs` does not trim spaces).

## Outcomes & Retrospective

As of 2026-09-28, milestones 1 through 7 are done, and milestone 8 waits on validation on macOS.
On Linux, the Cargo build, `python3 test.py`, and the integration tests pass, and the Buck build builds every target outside `third-party/rust/` apart from the two `//shed/completion_verify` packages that need `dnf`.
No code path sends data to a Meta service.
The tech-debt tracker lists what still ties the repository to upstream projects under "Upstream connections" (release downloads, the `com.facebook` packages, the Tpx result protocol, `INSIDE_RE_WORKER`, and `gen_bytecode_bundle.py`).
The rename waits for the owner's choice from the shortlist.

Some tests and targets passed only with Meta's defaults, such as hybrid Remote Execution for projects without execution platforms and jemalloc from the toolchain. Their failures after the removal gave generic messages, and each needed its mechanism traced before it was marked, fixed, or deleted.
`git grep` skips untracked files, so the sweeps of this work need `--untracked` to cover the new documents.

## Context and Orientation

- `docs/exec-plans/tech-debt-tracker.md` lists what still ties the repository to upstream projects under "Upstream connections".
- `CHANGELOG.md` lists the user-visible removals and renames.
- `.buckconfig`, `build_defs/`, `toolchains/`, and `third-party/` define the repository's Buck build, which `ARCHITECTURE.md` describes under "Two build definitions".
- `app/buck2_client/src/commands/init.rs` writes the cells of new projects.
- `website/config_impl.ts` configures the documentation site.
- `tests/conftest.py` and `tests/e2e_util/buck_workspace.py` hold the pytest harness, and `tests/README.md` describes it.

## Plan of Work

Milestones 2 through 6 edit separate parts of the tree and can proceed in any order. Milestone 8 runs last.

1. Rust (milestones 1 and 2). Delete `shed/scribe_client`, `app/buck2_explain`, `app/buck2_cmd_rage_client`, the `explain` command, and the HTML query output. Write the event log directly and delete `debug persist-event-logs` and `debug upload-re-logs`. Delete the Manifold client and `log_use_manifold`, and print local paths where Manifold URLs appeared. Delete the Scribe sink, `BuildGraphStats`, the health check crates, x2p, VPNless handling, `CertState`, internal certificates, tenting, and the Eden options. Reduce `metadata::collect()` to local fields. Remove `fbinit`, rust-project's Scuba logging, and the Meta remote execution settings. Keep the open-source branch of `is_open_source()`, `if_else_opensource!`, and `facebook_only()`, then delete them and `Applicability::Internal`. Prune the `buck2_data` messages left unused.
2. Prelude (milestone 3). Delete `is_full_meta_repo()` and the `meta_only` references, resolve the `@oss-disable` and `@oss-enable` markers, replace `fb_native` with `native`, and remove the `fbcode`, `fbsource`, `fbcode_macros`, `buck`, and `ovr_config` cell references. Delete `oncall` calls, Meta URLs, and Meta-only configuration keys, attributes, and JVM code.
3. Buck build (milestone 4). Move the toolchains, the Rust third-party definitions, protoc, and the Windows libraries out of `shim/` into `toolchains/`, `third-party/`, and `build_defs/`. Rewrite every BUCK file to load `build_defs/` and use `//app/...`, `//third-party/rust:...`, and `prelude//os:...` labels. Delete the rest of `shim/`, the unused Meta `.bzl` files, the `[oss]` configuration, and the `oncall` calls.
4. Tests (milestone 5). Add a pytest configuration, requirements, and a `conftest.py` under `tests/`. Rewrite the `buck2.tests.` imports, drop TPX and `__manifest__`, replace `fbpython` in test data, regenerate the golden files, and delete the Meta-only tests and harness files.
5. Documentation and metadata (milestone 6). Switch the website to `@docusaurus/preset-classic` and delete Google Analytics, Algolia, the internal-content tags, and the internal pages. Rewrite Meta text and links in `website/docs/`, `docs/`, the root files, `.github/`, and `.claude/`. Remove `oncall` calls and `com.facebook` package names from `examples/`.
6. Sweep (milestone 8). Remove the `none` aliases from `init.rs` and the example `.buckconfig` files once the prelude names none of those cells, regenerate golden files that depend on the final binary, update `AGENTS.md`, `ARCHITECTURE.md`, `docs/developers/basics.md`, and the tech-debt tracker, and complete this plan.

## Validation and Acceptance

Commands run from the repository root.

- `cargo build --bin=buck2` and `python3 test.py` pass after each Rust milestone.
- `buck2 targets //...` loads every package of the repository's Buck build, and `buck2 build //:buck2` builds the binary.
- The example projects load with `buck2 targets //...`.
- The pytest suite under `tests/` passes against `target/debug/buck2`, apart from tests marked as needing remote execution, cgroups, or helper binaries, and the tests whose prerequisites `tests/README.md` lists.
- A search for Meta identifiers (`fbcode`, `fbsource`, `internalfb`, `is_full_meta_repo`, `oss-disable`, `oss-enable`, `oncall(`, Scuba, Scribe, Manifold, analytics endpoints) finds only copyright headers, the attribution to the upstream project, and the `com.facebook` packages this plan keeps.

## Idempotence and Recovery

Each milestone edits files that Git tracks, and Git history restores them.
