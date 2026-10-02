# Tech-debt tracker

Each entry states a finding, the evidence for it, and the condition for removing it. [The plan contract](../PLANS.md) says when to add an entry.
Counts and results are from 2026-09-26 unless an entry gives another date. Commands run from the repository root.

## Build and verification

### The repository needs nightly Rust and tokio's unstable API

`rust-toolchain.toml` pins a nightly toolchain, and the crates enable 23 unstable features. On 2026-10-02, most had stable replacements of one or a few lines each, or mechanical rewrites (30 `try` blocks, 21 `box` patterns, 8 trait aliases, 10 `macro` definitions). Two features need API changes:
- `try_trait_v2` lets about 170 functions use `?` on `ExitResult`, `CommandOutcome`, and `ResultMaybeCompatible` (`app/yak_client_ctx/src/exit_result.rs`).
- `async_fn_traits` names `AsyncFnOnce::CallOnceFuture` in 15 `Send` bounds of `dice/dice/src/api/computations.rs`, which about 120 callers of `compute_join` and its relatives rely on.
`.cargo/config.toml` sets `--cfg tokio_unstable` because `app/yak_server/src/snapshot.rs` and `app/yak_daemon/src/daemon.rs` read 21 unstable runtime metrics. Three of them (the blocking queue depth and the counts of blocking threads) reach the superconsole IO header and the Chrome trace, and the rest reach only the event log.
`rustfmt.toml` sets unstable import options, and `.github/actions/build_release/action.yml` and `build_debug/action.yml` pass `-Z unstable-options --artifact-dir`.
Remove this entry when stable Rust and stable tokio provide these features, or when the code stops using them.

### The integration tests have not passed in CI

`.github/workflows/integration-tests.yml` runs `pytest tests` on Linux against a debug build, and no run of it has completed.
On 2026-09-28, the whole suite ran on Linux under Python 3.12 as a user other than root, with `ps` and `lldb` installed and the daemon in a cgroup below the root of its cgroup namespace.
That run gave 1726 passed, 230 skipped, 3 expected failures, and no other failures.
After the rename to yak, the same setup gave 1763 passed, 190 skipped, and 3 expected failures, with `YAK_COMPLETION_VERIFY` set so the completion tests ran.
On 2026-09-29, after the removal of the JVM, Android, and JavaScript support, the same setup without a completion helper gave 1728 passed, 223 skipped, and 3 expected failures.
After the removal of the Buck1 compatibility code, the same setup gave 1727 passed, 223 skipped, and 3 expected failures.
After the rename of the messages and comments of the code, the same setup gave 1727 passed, 223 skipped, and 3 expected failures.
The skipped tests need a Remote Execution backend, cgroup delegation, helper binaries, Go, or Watchman. The repository has no Remote Execution backend to test against.
A separate run with Go 1.26, `clang`, and `lld` passed the 30 tests in `tests/prelude/test_prelude_rules.py`, which include the 19 Go tests.
On 2026-10-02, the whole suite ran in `debian:bookworm-slim` with pytest under Python 3.12, Debian's Python 3.11 as `python3` for actions, clang 14, lld, Go 1.26.8, and the completion helper. It gave 1871 passed, 209 skipped, 3 expected failures, and 2 failures, in which `test_generate.py` built a build script that links a shared library through `CC`. The failures came from `from_any_dir.py`, which used an argument of `Path.relative_to` that Python 3.12 added, and from the `LD` shim, which let clang add `-pie` to a `-shared` link. After both fixes, the 212 tests of `tests/core/generate`, `tests/tools`, `tests/prelude`, `tests/core/external_cells`, `tests/core/test`, `tests/core/prelude`, and `tests/core/completion` passed.
Whether the GitHub runner puts the daemon in a cgroup below the root of its cgroup namespace is unverified.

Remove this entry when the workflow passes.

### The yak build of yak has not run in GitHub Actions

`.github/workflows/build-with-yak.yml` builds `//app/yak:yak` and `//app_dep_graph_rules:test_yak_dep_graph` on `ubuntu-latest`. Its commands succeeded in a Linux container only with `debug = "line-tables-only"`, because the compile of `starlark` with the full debug information of `[profile.dev]` peaked at 5.0 GB (`docs/exec-plans/completed/2026-10-02-build-yak-with-yak.md`). The job builds with 2 jobs to stay within the runner's 16 GB, and it relies on the disk that its `Free disk space` step frees. Neither limit has been measured on a runner.
Remove this entry when the workflow passes on `main`.

### Two Rust tests are compiled out

`app/yak_interpreter/src/dice.rs` compiles `pagable_starlark_test` only under `cfg(all(test, yak_build))`, and `create_minimal_for_test` in `app/yak_resource_control/src/cgroup.rs` returns `None` unless `cfg(yak_build)` is set. No build sets `yak_build`, so neither `cargo test` nor `yak test` runs these tests. The cgroup test also reads `PREP_CGROUP_SCRIPT`, which names `shed/cgroups/prep_cgroup.sh` and which no build sets.
Remove this entry when both tests run under `cargo test`, or when they are deleted.

### The `.elapsed()` ban is unenforced

`docs/developers/basics.md` bans `Instant::elapsed()`, but no configuration in this repository enforces the ban. `clippy.toml` has no entry for it.

`git grep -n '\.elapsed()' -- 'app/*.rs'` finds no calls.

Remove this entry when a lint rejects `.elapsed()` in `app/`, or when `docs/developers/basics.md` drops the convention.

### Lint levels have several copies

The Cargo lint levels live in `[workspace.lints]` in `Cargo.toml`.
Eight crates copy the whole table into their own `Cargo.toml` to add a `check-cfg` entry, and nothing checks that the copies match. `docs/developers/basics.md` lists the eight crates.
The macros in `build_defs/rust.bzl` set no lint levels, so the Yak build reports only the default lints of `rustc`.

Remove this entry when each build reads lint levels from one source, or when a check keeps the copies equal.

### Every Cargo command warns about an unused patch

`[patch.crates-io]` in `Cargo.toml` patches `bindgen`, which no crate in the Cargo workspace uses.

`cargo build --bin=yak` prints ``patch `bindgen v0.72.1 (...)` was not used in the crate graph``.

Remove this entry when the warning no longer appears.

### `yak_miniperf_test` runs no test

`//app/yak_miniperf:yak_miniperf_test` is a `rust_library` over `app/yak_miniperf/test/lib.rs`, and that file compiles only under `cfg(test)`, so the target builds an empty library. `app/yak_miniperf/Cargo.toml` has no target for the file.
The test needs `MINIPERF` and `THREE_BILLION_INSTRUCTIONS` in its environment and the `anyhow`, `bincode`, `tempfile`, and `yak_miniperf_proto` crates. Meta's `rust_library` macro made a test target from `test_deps` and `test_env`, and the macros in this repository never did.

Remove this entry when a Yak or Cargo target runs the test.

### `test_perf_thread_instruction_counter` fails where perf events are denied

`per_thread_instruction_counter::tests::test_perf_thread_instruction_counter` in `app/yak_util/src/per_thread_instruction_counter.rs` unwraps the result of `PerThreadInstructionCounter::init()`.
Where the host denies `perf_event_open`, `init()` returns `Operation not permitted`, and the test panics.
The test returns early when `GITHUB_ACTIONS` is set, because it fails with permission denied on GitHub's runners.
The build file interpreter continues without the counter when `init()` fails (`app/yak_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`).
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
The docs job in `.github/workflows/upload_yak.yml` runs `yarn` and `yarn build_prebuilt` in `website/`.

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

### `cas_artifact` has no test

The `cas_artifact` rule fetches a blob or directory tree from the Remote Execution CAS by a digest that the build file names.
Its only test, `test_cas_artifact`, built targets whose digests name content in Meta's CAS with BLAKE3-KEYED digests, so the fork removed it, and it removed it again on 2026-09-30 after a port brought it back.
Remove this entry when a test uploads known content to a test CAS and builds a `cas_artifact` target that fetches it.

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

### Build scripts on Linux cannot link a static executable through `CC`

The `CC` that `buildscript_run` gives build scripts links through `--ld-path=<LD>`, and `LD` runs the toolchain's linker driver with each argument passed through as `-Wl,<arg>` (`prelude/rust/cargo_buildscript.bzl`). For `clang -static`, the driver behind `LD` receives `-static` only as a linker argument, so it adds a dynamic loader to a link of static startup files. On 2026-10-02, in `debian:bookworm-slim` with clang 14 and lld, `clang -static --ld-path=<LD> m.c s.c` linked a program that crashed at startup, and plain `clang -static` linked one that ran. Executables and shared libraries link correctly.
Remove this entry when `CC` links static executables that run.

### Every command waits for the tool identities

At the start of each command, the daemon runs `rustc -vV`, `go version`, and `clang --version` and waits for all three (`app/yak_server/src/tool_identity.rs`). On 2026-10-01, on an aarch64 macOS machine, the three took a median of 21 ms in parallel, and a no-op `yak build` took a median of 34 ms with them.
Remove this entry when the daemon computes the identities without running the tools on every command, or a measurement shows their cost is below 5% of a no-op build.

### The system C++ toolchain on Windows carries the identity of clang

`system_toolchains()` passes `tool_identity.clang` to `system_cxx_toolchain`, which runs the MSVC tools of `prelude//toolchains/msvc:msvc_tools` on Windows. An upgrade of MSVC leaves the keys of C and C++ actions unchanged there. The archiver of the toolchain (`ar` on other systems) carries no identity.
Remove this entry when the Windows toolchain carries an identity of the MSVC compiler.

### The C++ toolchain of this repository carries no tool identity

`toolchains/YAK` declares `system_cxx_toolchain` with `gcc` and `g++`, and the daemon computes identities only for `rustc`, `go`, and `clang` (`app/yak_server/src/tool_identity.rs`). An upgrade of gcc leaves the keys of the actions that run `gcc` or `g++` unchanged, such as the links of Rust binaries.
Remove this entry when the toolchain carries an identity of its compiler.

### The C compiles of the Go standard library miss a shared cache

On 2026-10-01, two checkouts of the GitHub CLI at different paths shared 4373 of 4390 build actions through bazel-remote. The other 17 were the `c_compile` actions of `prelude//go/tools:stdlib` for the cgo files of the runtime, such as `goroot/src/runtime/cgo/gcc_unix.c`, whose action digests differ between the checkouts.
Remove this entry when those actions get the same digest in two checkouts of one project.

### A daemon without a Remote Execution backend waits 45 seconds to fail

If an execution platform enables remote execution and no backend answers, each daemon start waits about 45 seconds before the command fails.
`ReConnectionManager::new` in `app/yak_server/src/daemon/state.rs` allows 10 connection attempts, and `new_retry` in `app/yak_execute/src/re/client.rs` sleeps 1, 2, and up to 9 seconds between them.
In the integration test suite, 23 tests outside the Go tests take 45 seconds or more for this reason.

Remove this entry when a missing backend fails the command without the retry delay, or the delay is configurable.

### A failed daemon start waits out the startup timeout

When daemon initialization fails, the client waits for the whole startup timeout before it reports the error.
`test_init_data_timeout` and `test_daemon_startup_error` in `tests/core/build/test_error_categorization.py` each take about 120 seconds.

Remove this entry when the client reports an initialization failure as soon as the daemon exits.

### `yak install` seemingly leaves the installer running after a failed build

The installer starts in the `try_compute2` call in `app/yak_server_commands/src/install.rs`, and nothing stops it when the build side fails, such as on a validation failure.

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
For a rename, `app/yak_file_watcher/src/notify.rs` passes the renamed path to `file_added_or_removed` and `dir_added_or_removed` in `app/yak_common/src/file_ops/dice.rs`.
Those calls invalidate the path's metadata and its parent's listing, but not the directory's own listing or the build files under it, so a query against the old path reads the cached listing and build file.

On macOS on 2026-09-27, a project had a `.yakconfig` that sets only `[cells] root = .` and a `pkg/YAK` that defines one target `a` with a rule returning `DefaultInfo()`.
`yak uquery //pkg/...` printed `root//pkg:a`.
After `mv pkg pkg2`, the daemon reported two file change events.
Seven seconds later, `yak uquery //pkg/...` still printed `root//pkg:a` and exited 0.
After `yak kill`, the same query failed because `pkg` does not exist. The Linux behavior was not checked.

Remove this entry when the query after the rename fails without a daemon restart.

### A command can miss a file changed just before it starts

With the default `notify` file watcher, a command that starts milliseconds after a file is created or edited can miss the change until the next command.
`NotifyFileWatcher::sync2` in `app/yak_file_watcher/src/notify.rs` takes the events that have arrived when the command starts, and macOS delivers FSEvents asynchronously, so an event can arrive after the sync.

On macOS on 2026-09-30, a loop created a file in a package of a Go module, ran `yak build //...` or `yak targets //...`, deleted the file, and ran the command again.
In 17 rounds over 3 runs of the loop, 5 first commands did not see the new file. In 1 of those rounds the next command saw it, and in the other 4 the creation and the deletion both went unseen.
Later runs of 8 and 20 rounds missed none. The Linux behavior was not checked.
On 2026-10-01, `tests/core/build/test_modify.py::test_modify_genrule_notify` failed once in a full run of the integration suite on macOS, where the build after an edit printed the old contents of the file, and it passed in 3 reruns.

Remove this entry when the sync waits for the events of changes made before the command started, as Watchman's sync cookie does.

### Go cells support no workspace, vendoring, or local replacement

`app/yak_external_cells/src/go.rs` runs `go list` with `GOWORK=off`, so a `go.work` file does not apply, and it fails when the module has a `vendor/modules.txt`.
`generate` in `app/yak_external_cells_go/src/generate.rs` fails when a `replace` directive names a local directory, because the cell would copy that directory without reading its files through DICE.
`test_go_cell_rejects_a_vendored_module` in `tests/core/external_cells/test_go.py` checks the vendoring error.

Remove this entry when a go cell builds a module of a `go.work` file, a vendored module, and a module with a local replacement.

### Go cells resolve dependencies with cgo enabled

`app/yak_external_cells/src/go.rs` runs `go list` with `CGO_ENABLED=1` for each platform.
The prelude's Go rules enable cgo only when a C++ toolchain is available (`cgo_enabled` in `prelude/decls/go_common.bzl`), so a package built without cgo can import a package that only its non-cgo files name, and that dependency is missing from its target.

Remove this entry when the dependencies of a go cell's targets select on whether cgo is enabled.

### A Go test cannot rebuild a dependency against the package under test

A package with internal tests whose external tests import a package that imports it fails analysis with `conflict for package` in `merge_pkgs` (`prelude/go/packages.bzl`).
`go test` compiles such a dependency again against the package with its internal tests, and `go_test` links the dependency as its own target builds it.
`go_test` with `external_tests_only` covers the packages without internal tests, such as `pkg/cmd/auth/shared/gitcredentials` in the GitHub CLI.

Remove this entry when `go_test` builds the dependencies of external tests that import the package under test against its test variant.

### Go binaries carry no module version

`go build` stamps the main module's version, taken from Git, into the build information that `debug.ReadBuildInfo` returns, and the prelude's `go_binary` does not.
On 2026-10-01, `gh version` printed `gh version DEV` for the GitHub CLI built by yak and `gh version 2.102.0+dirty` for the one `go build` built.

Remove this entry when a `go_binary` reports its module's version as `go build` does, or the documentation says how to stamp one.

### Go module paths that differ only in case share a directory on macOS

A go cell copies each third-party module version to a directory named `<module path>@<version>` in `yak-out`.
Go's module cache writes an upper-case letter as `!` and its lower-case letter, because macOS file systems ignore case by default, and the cell does not.
Two module paths that differ only in case would share a directory there. No project has shown it.

Remove this entry when the cell escapes module paths as the module cache does.

## Upstream connections

The repository owner plans to remove what still ties the repository to Meta's upstream projects. Links that credit upstream issues, pull requests, and projects stay.

### Downloads from upstream releases

- `.github/workflows/release.yml` and `.github/workflows/upload_yak.yml` publish DotSlash files with the `facebook/dotslash-publish-release` action.

`git grep -n -E 'github\.com/facebook(incubator)?/[^/]+/releases|facebook/dotslash-publish-release'` lists them.

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
