# Tech-debt tracker

Each entry states a finding, the evidence for it, and the condition for removing it. [The plan contract](../PLANS.md) says when to add an entry.
Counts and results are from 2026-09-26 unless an entry gives another date. Commands run from the repository root.

## Build and verification

### The Buck build fails on macOS

On macOS, `target/debug/yak build //:yak` fails at `//third-party/rust:objc2-0.6`.
`objc2` 0.6.4 reads `CARGO_PKG_VERSION` at compile time, and `third-party/rust/fixups/objc2/fixups.toml` does not exist, so the rule that `reindeer` generates does not set the variable.
`reindeer buckify` also warns that eight other crates have build scripts but no fixups (`alloca`, `bindgen`, `clang-sys`, `constant_time_eq`, `icu_locale_fallback_data`, `icu_segmenter_data`, `psm`, `stacker`).

The macOS failure is from 2026-09-27, when the third-party definitions still lived in `shim/third-party/rust`. The build stopped at that failure, so later failures are unknown.
On Linux on 2026-09-28, after the rename to yak, `./bootstrap/reindeer --third-party-dir third-party/rust buckify` followed by `target/debug/yak build //:yak` succeeded.

Remove this entry when `yak build //:yak` succeeds on macOS after a fresh `buckify`.

### The integration tests have not passed in CI

`.github/workflows/integration-tests.yml` runs `pytest tests` on Linux against a debug build, and no run of it has completed.
On 2026-09-28, the whole suite ran on Linux under Python 3.12 as a user other than root, with `ps` and `lldb` installed and the daemon in a cgroup below the root of its cgroup namespace.
That run gave 1726 passed, 230 skipped, 3 expected failures, and no other failures.
After the rename to yak, the same setup gave 1763 passed, 190 skipped, and 3 expected failures, with `BUCK2_COMPLETION_VERIFY` set so the completion tests ran.
On 2026-09-29, after the removal of the JVM, Android, and JavaScript support, the same setup without a completion helper gave 1728 passed, 223 skipped, and 3 expected failures.
After the removal of the Buck1 compatibility code, the same setup gave 1727 passed, 223 skipped, and 3 expected failures.
The skipped tests need a Remote Execution backend, cgroup delegation, helper binaries, Go, or Watchman. The repository has no Remote Execution backend to test against.
A separate run with Go 1.26, `clang`, and `lld` passed the 30 tests in `tests/prelude/test_prelude_rules.py`, which include the 19 Go tests.
Whether the GitHub runner puts the daemon in a cgroup below the root of its cgroup namespace is unverified.

Remove this entry when the workflow passes.

### The crate dependency rules run only in the Buck build

`//app_dep_graph_rules:test_buck2_dep_graph` checks the rules in `app_dep_graph_rules/rules.bzl` during analysis, and `yak build //app_dep_graph_rules:test_buck2_dep_graph` succeeded on Linux on 2026-09-28.
CI runs no Buck build, so a change that breaks a rule passes CI.

Remove this entry when CI runs the check.

### The `.elapsed()` ban is unenforced

`docs/developers/basics.md` bans `Instant::elapsed()`, but no configuration in this repository enforces the ban. `clippy.toml` has no entry for it.

`git grep -n '\.elapsed()' -- 'app/*.rs'` finds no calls.

Remove this entry when a lint rejects `.elapsed()` in `app/`, or when `docs/developers/basics.md` drops the convention.

### Lint levels have several copies

The Cargo lint levels live in `[workspace.lints]` in `Cargo.toml`.
Eight crates copy the whole table into their own `Cargo.toml` to add a `check-cfg` entry, and nothing checks that the copies match. `docs/developers/basics.md` lists the eight crates.
The macros in `build_defs/rust.bzl` set no lint levels, so the Buck build reports only the default lints of `rustc`.

Remove this entry when each build reads lint levels from one source, or when a check keeps the copies equal.

### Every Cargo command warns about an unused patch

`[patch.crates-io]` in `Cargo.toml` patches `bindgen`, which no crate in the Cargo workspace uses.

`cargo build --bin=yak` prints ``patch `bindgen v0.72.1 (...)` was not used in the crate graph``.

Remove this entry when the warning no longer appears.

### `yak build //...` fails in third-party crates

`//...` includes every crate that `reindeer buckify` generates in `third-party/rust/YAK`, and some of them fail to build on Linux.
On 2026-09-28, `buck2 build //third-party/... --keep-going` failed in 10 targets:

- The eight `protoc-bin-vendored` platform crates read `CARGO_MANIFEST_DIR` at compile time, and they have no fixups.
- The build script of `openssl-sys` reads `CARGO_PKG_VERSION` at compile time, and `third-party/rust/fixups/openssl-sys/fixups.toml` does not set it.
- `bindgen` includes a file from `OUT_DIR`, and it has no fixup that runs its build script.

`--keep-going` skips the crates that depend on these targets, and those crates can fail too.
On Linux, `buck2 build` of the 265 targets outside `third-party/rust/` fails only in the two targets that the next entry describes, so none of those targets needs these crates. The Buck build takes `protoc` from `third-party/proto/`.

Remove this entry when `yak build //...` succeeds, or when the documentation names the target pattern that the Buck build supports.

### `//shed/completion_verify` needs `dnf`

`download_rpm` in `shed/rpm_download/packages.bzl` runs `dnf download` and `rpm2archive`, so `//shed/completion_verify/packages:zsh` and `//shed/completion_verify/packages:fish` build only where those tools exist, such as on Fedora.
On Linux, `//shed/completion_verify:completion_verify` takes both packages as resources.
The completion tests need the binary through `YAK_COMPLETION_VERIFY`, and they skip without it (`tests/README.md`).
On Linux on 2026-09-28, a `completion_verify` built with Cargo from `shed/completion_verify/src/` ran them. Its `completion_verify.resources.json` pointed at directories that link the Debian `fish` and `zsh` into the layout of the RPMs.

Remove this entry when the completion packages build without `dnf`.

### `buck2_miniperf_test` runs no test

`//app/buck2_miniperf:buck2_miniperf_test` is a `rust_library` over `app/buck2_miniperf/test/lib.rs`, and that file compiles only under `cfg(test)`, so the target builds an empty library. `app/buck2_miniperf/Cargo.toml` has no target for the file.
The test needs `MINIPERF` and `THREE_BILLION_INSTRUCTIONS` in its environment and the `anyhow`, `bincode`, `tempfile`, and `buck2_miniperf_proto` crates. Meta's `rust_library` macro made a test target from `test_deps` and `test_env`, and the macros in this repository never did.

Remove this entry when a Buck or Cargo target runs the test.

### `test_perf_thread_instruction_counter` fails where perf events are denied

`per_thread_instruction_counter::tests::test_perf_thread_instruction_counter` in `app/buck2_util/src/per_thread_instruction_counter.rs` unwraps the result of `PerThreadInstructionCounter::init()`.
Where the host denies `perf_event_open`, `init()` returns `Operation not permitted`, and the test panics.
The test returns early when `GITHUB_ACTIONS` is set, because it fails with permission denied on GitHub's runners.
The build file interpreter continues without the counter when `init()` fails (`app/buck2_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`).
On Linux on 2026-09-28, the test failed this way in a container, which stopped `python3 test.py`.

Remove this entry when the test skips or passes wherever `perf_event_open` is denied.

### The paging tests of `starlark` fail when they run in parallel

Tests in `starlark-rust/starlark/src/pagable/tests.rs` read `starlark_partial_deser_stats()` before and after a page-in and assert on the difference.
The whole test process shares the counters, so a test that pages in while another test runs adds its reads to the other test's difference.
On macOS on 2026-09-28, 9 of 10 runs of `cargo test -p starlark --features pagable --lib` failed one or two of these tests, and five different tests failed across the runs.
Each of the five passed in 10 of 10 runs alone, and the `pagable::tests` module passed in 5 of 5 runs with `--test-threads=1`.
The failures stopped `python3 test.py`.

Remove this entry when the tests pass under the default parallelism of `cargo test`.

### The documentation site build is unverified

On 2026-09-28, `website/package.json` dropped Meta's internal docs preset, Google Analytics, and Algolia for `@docusaurus/preset-classic`, and a script edited `website/yarn.lock` and `website/package-lock.json` to match without running a package manager.
The docs job in `.github/workflows/upload_buck2.yml` runs `yarn` and `yarn build_prebuilt` in `website/`.

Remove this entry when that job succeeds.

### The documentation site has no search

The Algolia configuration in `website/config_impl.ts` pointed at Meta's index and was removed, and the site has no other search. There is no proposal yet.

Remove this entry when the site has a search box.

### Examples that fail to load or build

- `examples/toolchains/cxx_zig_toolchain` fails to build with `error: unable to parse command line parameters: NestedResponseFile`. Zig 0.11.0 rejects the nested response files that the prelude's compile argument file uses. `.github/workflows/build-and-examples.yml` sets `continue-on-error` for it.
- In `examples/with_prelude`, `yak targets //...` fails in `root//third-party/haskell:rts` while coercing `cxx_header_dirs`.
- In `examples/bxl_tutorial`, `yak targets //...` fails while evaluating `prelude//erlang/erlang_otp_application.bzl`, because the project defines no `toolchains` cell.
- `examples/no_prelude/toolchains/go_toolchain.bzl` downloads the `linux-amd64` Go on every Linux host, so `yak build //...` in `examples/no_prelude` fails at `root//go:main` on Linux on ARM. It failed the same way before the rename to yak.

The first three fail the same way at `903bfd7a61`.

Remove each item when its command succeeds.

## Defects

### `apple_test` cannot run under the built-in test runner

`_get_test_info` in `prelude/apple/apple_test.bzl` returns an `ExternalRunnerTestInfo` whose command is `false` and passes the test bundle through environment variables, because Meta's Tpx test runner built the real command.
The built-in runner runs `false`, which exits 1.

Remove this entry when `apple_test` produces a command that runs the test bundle.

### Split debug info passes an fbcc flag to clang

With `split_debug_mode = "split"` on a `cxx_toolchain` and a clang compiler, `prelude/cxx/compile.bzl` adds `--fbcc-create-external-debug-info=<path>` to the compile command.
Only fbcc, Meta's compiler wrapper, understands the flag, so a plain clang rejects it. The default `split_debug_mode` is `none`.
`prelude/cxx/tools/clang_tidy_wrapper.py` filters fbcc flags out of compile commands, and `prelude/cxx/dist_lto/tools/dist_lto_opt_gnu.py` expects the compiler command to start with the fbcc wrapper and a `--cc=` flag.

Remove this entry when split debug info uses flags that clang accepts and no tool expects the fbcc command layout.

### A daemon without a Remote Execution backend waits 45 seconds to fail

If an execution platform enables remote execution and no backend answers, each daemon start waits about 45 seconds before the command fails.
`ReConnectionManager::new` in `app/buck2_server/src/daemon/state.rs` allows 10 connection attempts, and `new_retry` in `app/buck2_execute/src/re/client.rs` sleeps 1, 2, and up to 9 seconds between them.
In the integration test suite, 23 tests outside the Go tests take 45 seconds or more for this reason.

Remove this entry when a missing backend fails the command without the retry delay, or the delay is configurable.

### A failed daemon start waits out the startup timeout

When daemon initialization fails, the client waits for the whole startup timeout before it reports the error.
`test_init_data_timeout` and `test_daemon_startup_error` in `tests/core/build/test_error_categorization.py` each take about 120 seconds.

Remove this entry when the client reports an initialization failure as soon as the daemon exits.

### `yak install` seemingly leaves the installer running after a failed build

The installer starts in the `try_compute2` call in `app/buck2_server_commands/src/install.rs`, and nothing stops it when the build side fails, such as on a validation failure.

Remove this entry when a failed build stops the installer.

### The `fs_hash_crawler` file watcher misses changes

With `yak.file_watcher = fs_hash_crawler`, a symlink whose target changes is not reported as changed, and a file name that contains a backslash fails the command.
The strict `xfail` markers on two tests in `tests/core/build/test_symlinks.py` and one in `tests/core/build/test_uncategorized.py` record both bugs.

Remove this entry when those tests pass without the markers.

### Local actions outlive a killed daemon

`yak kill` leaves the daemon's running local actions behind on macOS and Linux.
Runs of `tests/core/daemon/test_concurrency.py` and `tests/core/daemon/test_daemon.py` left `python3` processes from test actions running on both systems, 42 of them on macOS.
On Linux on 2026-09-28, `tests/core/daemon/test_concurrency.py` left 13 such processes before the rename to yak, and the whole suite left 13 after it.

Remove this entry when killing the daemon stops its local actions.

### Apple rules select on configuration the prelude does not define

The Apple rules select on `config//os/sdk/apple/constraints:*`, `config//version:*`, `config//runtime/constraints:maccatalyst`, `config//runtime/constraints:runtime`, `config//cpu/constraints:universal`, and `config//cpu/constraints:universal-enabled`.
`yak init` aliases `config` to `prelude`, and the prelude has no `os/sdk/apple/constraints` or `version` package, and its `runtime/constraints` and `cpu/constraints` packages lack those targets.
`CONSTRAINT_PACKAGE` in `prelude/platforms/apple/build_mode.bzl` names `prelude//platforms/apple/constraints`, which does not exist, and `APPLE_PLATFORMS_MAP` in `prelude/platforms/apple/platforms_map.bzl` is empty.
All of these are missing at `903bfd7a61` too.

`git grep -h -o -E 'config//(os/sdk/apple|version|runtime/constraints|cpu/constraints)[^"]*' -- prelude | sort -u` lists the labels.

Remove this entry when every label the Apple rules select on exists in the prelude, or the documentation says which labels a project's `config` cell must define.

### A running daemon keeps the packages of a renamed directory

With the default `notify` file watcher, a directory renamed under a running daemon keeps its packages at the old path until the daemon restarts.
For a rename, `app/buck2_file_watcher/src/notify.rs` passes the renamed path to `file_added_or_removed` and `dir_added_or_removed` in `app/buck2_common/src/file_ops/dice.rs`.
Those calls invalidate the path's metadata and its parent's listing, but not the directory's own listing or the build files under it, so a query against the old path reads the cached listing and build file.

On macOS on 2026-09-27, a project had a `.buckconfig` that sets only `[cells] root = .` and a `pkg/BUCK` that defines one target `a` with a rule returning `DefaultInfo()`.
`buck2 uquery //pkg/...` printed `root//pkg:a`.
After `mv pkg pkg2`, the daemon reported two file change events.
Seven seconds later, `buck2 uquery //pkg/...` still printed `root//pkg:a` and exited 0.
After `buck2 kill`, the same query failed because `pkg` does not exist. The Linux behavior was not checked.

Remove this entry when the query after the rename fails without a daemon restart.

## Upstream connections

The repository owner plans to remove what still ties the repository to Meta's upstream projects. Links that credit upstream issues, pull requests, and projects stay.

### Downloads from upstream releases

- `bootstrap/reindeer` downloads `reindeer` from `facebookincubator/reindeer` releases, and `.github/actions/setup_reindeer/action.yml` installs it from that repository with `cargo install`.
- `.github/workflows/release.yml` and `.github/workflows/upload_buck2.yml` publish DotSlash files with the `facebook/dotslash-publish-release` action.

`git grep -n -E 'github\.com/facebook(incubator)?/[^/]+/releases|facebook/dotslash-publish-release|facebookincubator/reindeer reindeer'` lists them.

Remove each item when the fork publishes its own artifact or drops the download.

### The Tpx result protocol

The Common Test hooks in `prelude/erlang/common_test/` write results for Meta's Tpx test runner.
The built-in test runner reads exit codes, so it uses none of that output.

Remove this entry when the hooks report results only in a form the built-in runner reads.

### `INSIDE_RE_WORKER` checks

`prelude/apple/tools/swift_exec.py`, `prelude/erlang/common_test/common/src/test_artifact_directory.erl`, and `prelude/toolchains/apple/xcode_version_checker/src/xcode_version_checks.m` detect remote execution through `INSIDE_RE_WORKER`, which Meta's Remote Execution workers set.
Other backends do not set it, so those tools behave as they do in local execution.
The checked-in `prelude/toolchains/apple/xcode_version_checker/xcode_version_checker` binary is built from that source by its `Makefile`.

Remove this entry when the tools detect remote execution in a way other backends support, or drop the checks.

### `gen_bytecode_bundle.py`

`prelude/python/tools/gen_bytecode_bundle.py` writes bytecode bundles for a `__par__.bytecode_bundle` loader, which `prelude/python/runtime/__par__/` does not contain.
`prelude/python/tools/YAK` exports the script and gives it to the `tool_tests` target, and only `prelude/python/tools/tests/gen_bytecode_bundle_test.py` uses it.

Remove this entry when the script and its test are deleted, or a rule runs the script.

### Comments and examples describe Buck1 and the removed JVM rules

`docs/exec-plans/completed/2026-09-29-remove-jvm-and-buck1-compatibility.md` removed the Buck1 compatibility code that did nothing or had a replacement.
The owner chose to keep the Buck1 behaviors that have no replacement, such as the working directory of tests and the `%s` substitution of multiple queries (2026-09-29).
Comments in code still explain those behaviors and others by comparison with Buck1. `git grep -n -i -E 'buck ?1|buck v1' -- app prelude dice starlark-rust` finds 68 lines in 47 files.
Some examples still use the removed JVM and Android support:

- The `genrule` examples in `prelude/decls/core_rules.bzl` build an `AndroidManifest.xml`.
- The `deps_query` example in `prelude/decls/common.bzl` filters Java annotation processors.

The second part of milestone 6 of `docs/exec-plans/active/2026-09-28-rename-the-fork.md` rewrites the comments and examples of the code.

Remove this entry when no comment explains yak's behavior by comparison with Buck1 and every example uses a rule that the prelude defines.
