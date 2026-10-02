# Build yak with yak

Follows [the plan contract](../../PLANS.md).

## Purpose

`yak build //app/yak:yak` in this repository builds the yak binary from the `YAK` files that `yak generate` writes from the workspace's `Cargo.toml` files, with the `cargo` cell supplying the third-party crates. The binary passes the integration tests that the Cargo-built binary passes. The repository is the largest test of yak's Rust support: 142 crates, 11 protobuf build scripts, C libraries, and a build script that embeds the prelude.

Before this plan, the yak build of this repository did not load, because its hand-written `YAK` files named the crates of a `third-party/rust` package whose generator had been removed. The crate dependency check `//app_dep_graph_rules:test_yak_dep_graph` does not run while the build does not load (tech-debt tracker, "The crate dependency rules run only in the Yak build").

The repository owner requires the self-build (2026-10-02).

## Progress

- [x] Prototype: run `yak generate` on a copy of the repository and build `//app/yak:yak`, working around each failure to reach the next (2026-10-02, Surprises & Discoveries).
- [x] Decide how the prelude reaches `yak_external_cells_bundled` (2026-10-02, Decision Log).
- [x] Plan of Work item 1: `yak generate` skips ignored directories (2026-10-02).
- [x] Plan of Work item 2: the `cargo` cell applies the `rustflags` of Cargo's configuration (2026-10-02).
- [x] Plan of Work item 3: a workspace member's build script runs in the member's directory of a tree with the workspace's layout, the member's files, and the files of `include` (2026-10-02). `test_generate_runs_a_build_script_in_the_workspace_layout` covers it. The protobuf crates of this repository build this way once they declare their `.proto` files (item 6).
- [x] Plan of Work item 4: build scripts get `DEP_<links>_<key>` from the build scripts of their normal dependencies with `links` (2026-10-02). `test_generate_passes_links_metadata_to_build_scripts` covers it, and a workspace that depends on `aws-lc-rs` 1.18.1 builds and runs.
- [x] Plan of Work item 5: members' crates name their files relative to the workspace's directory, through `srcs_path` of the Rust rules (2026-10-02). `test_generate_runs_a_build_script_in_the_workspace_layout` checks `file!()`.
- [x] Plan of Work item 6: the repository builds from generated `YAK` files (2026-10-02). On macOS, `yak build //app/yak:yak //app/yak:yak_client-bin //:yak_bundle //app_dep_graph_rules:test_yak_dep_graph` succeeds, and the check rejects a dependency of `yak_test_runner` on `yak_cmd_debug_client`. With `YAK_BINARY` set to the self-built binary, `tests/core/build`, `tests/core/test`, `tests/core/prelude`, and `tests/core/generate` gave 434 passed, 39 skipped, 3 expected failures, and 1 failure, `test_many_rebound_outputs_incremental_rebuild` (finding 12). `test_generate_includes_the_files_of_another_cell` covers `include` of another cell's `source_listing`. The 4 crates that joined the workspace build with yak, `yak test //dice/fuzzy_dice: //starlark-rust/benchmark_memory:` passes, and the 37 tests of `tests/core/completion` pass on macOS with `YAK_COMPLETION_VERIFY` set to the Cargo-built `completion_verify`.
- [ ] Plan of Work items 7 and 8.

## Surprises & Discoveries

The prototype ran on 2026-10-02 against a copy of `dfdf0496bf` on macOS, with `yak generate --force` in the copy's root. Each finding below was found by a failure, worked around in the copy, and followed by another build.

1. `yak generate` writes in 0.27 seconds. It replaced 144 hand-written `YAK` files and wrote a root `YAK` file that calls `cargo_workspace()`.
2. `yak generate` also wrote `YAK` files and `.yakconfig` cells for 5 Go modules under `tests/`, which `[project] ignore` lists. It ignores `[project] ignore`.
3. Loading failed in every package with "`superconsole-0.3.0` is a path dependency outside the workspace", because `Cargo.toml` excludes `superconsole` and `superconsole/Cargo.toml` declares its own `[workspace]`. As a member without its `[workspace]` section, all 157 packages load: 644 targets in 0.96 seconds from a cold daemon, plus 3379 targets in the `crates` cell.
4. The replaced files held targets that `cargo_package()` does not declare: 11 `rust_protobuf_library`, `proto_srcs`, 8 `rust_linkable_symbol`, `export_file`, `filegroup`, `constraint`, the root's `pagable_transition_alias` named `yak` and `yak_bundle`, and the `bundled_cell` of `app/yak_external_cells_bundled`. The `rust_linkable_symbol` targets are unneeded, because `app/yak_cmd_completion_client/src/completion.rs` reads the same files with `include_str!` outside `cfg(yak_build)`.
5. The vendored `protoc` of `protoc-bin-vendored` runs from the cell's copy of the crate, and the C libraries of `ring` and `aws-lc-sys` compile.
6. 5 protobuf build scripts fail because they read `.proto` files of sibling crates through relative paths, such as `../yak_host_sharing_proto` in `app/yak_data/build.rs`. `include` of `cargo_package()` puts the files in the sources of the build script's compile action, but the build script runs in a copy of its own package's files (`--manifest-dir` of `buildscript_run`), which holds neither the included files nor the workspace layout that `../` needs. The Cargo page states that `include` covers "files that a build script reads", so the behavior contradicts the documentation.
7. `app/yak_external_cells_bundled/build.rs` reads `../../prelude`. `include` takes only targets of the root cell, and this repository's `prelude` cell is the copy bundled in the running binary (`[external_cells] prelude = bundled`), so no target names the files of `prelude/`. A copy of the prelude inside the package does not help, because the directories of the prelude have build files and so form their own packages.
8. `aws-lc-rs`'s build script panics with "missing DEP_AWS_LC_ include". Cargo passes `DEP_<links>_<key>` variables from the build script of a package with `links` to the build scripts of the packages that depend on it, and neither the cell nor `buildscript_run` passes them. The cell sets only `CARGO_MANIFEST_LINKS`.
9. `aws-lc-rs` is in the graph only through the default features of `rustls` and `hyper-rustls`. `app/yak_certs/src/certs.rs` installs `rustls::crypto::ring::default_provider()`, so yak compiles two TLS crypto libraries and uses `ring`. With `default-features = false` and the features yak uses, the lock file loses 40 lines and `aws-lc-rs` leaves the graph.
10. The cell ignores `[build] rustflags` of `.cargo/config.toml`, which sets `--cfg tokio_unstable`. It evaluates `cfg(...)` conditions with plain `rustc --print cfg --target <triple>` (`platforms` in `app/yak_external_cells/src/cargo.rs`), so tokio lost its `cfg(tokio_unstable)` dependency on `tracing`, and it compiles crates without the flag, so `yak_server` and `yak_daemon` lost tokio's unstable metrics. A `rustc` wrapper that adds the flag to `--print cfg`, with the flag in the Rust toolchain's `rustc_flags`, emulated Cargo's behavior, and `//app/yak:yak` then built (307 seconds, after earlier builds had built most crates).
11. With the workarounds of findings 3, 6, 7, 9, and 10, the self-built binary passed 427 of 430 tests of `tests/core/build`, `tests/core/test`, `tests/core/prelude`, and `tests/core/generate` (`YAK_BINARY` set to it). `test_action_error` and `test_yak_fail` of `tests/core/build/test_error_categorization.py` failed because source paths are doubled, such as `yak_build_api/app/yak_build_api/src/actions/errors/action_error.rs`, where the Cargo build gives `yak_build_api/src/...`. The cell compiles a member from a copy that nests the workspace path below the package, which changes `file!()`, panic locations, and error tags.
12. `test_many_rebound_outputs_incremental_rebuild` of `tests/core/build/actions/test_dynamic_output.py` took 612 seconds with the self-built binary and 116 seconds with the Cargo-built one. `yak cquery 'deps(//app/yak:yak)'` took 8.2 and 9.1 seconds, so graph evaluation is not slower. `Cargo.toml` sets `opt-level = 1` for the `dev` profile, which the cell does not apply. The self-built binary is 510 MB and the Cargo-built one 205 MB. On 2026-10-02, run alone, the test passed in 91 seconds with a self-built binary compiled with `-Copt-level=1` in the Rust toolchain's `rustc_flags`, and in 117 seconds with the Cargo-built binary. With the binary compiled at rustc's default opt-level 0, it reached its 600-second timeout. The opt-level 1 binary is 185 MB. The cell also ignores `panic = "abort"` of the `dev` profile, so a panic in the self-built binary unwinds.
13. `toolchains/YAK` declares the repository's toolchains by hand, so its actions get no tool identities (`docs/exec-plans/completed/2026-10-01-key-actions-on-their-tools.md`).
14. 5 crates outside the workspace keep hand-written `YAK` files that name `//third-party/rust` targets: `dice/fuzzy_dice`, `shed/cgroups/use_some_memory`, `shed/completion_verify`, `starlark-rust/benchmark_memory`, and `starlark-rust/starlark/fuzz`.
15. `crates//:nix` does not exist, because `Cargo.lock` holds two versions of `nix`. The cell declares an alias without a version only for a crate with one version in the lock, so a hand-written target names `crates//:nix-0.31.3`.
16. `//app_dep_graph_rules:test_yak_dep_graph` failed on `root//app/yak:yak-lib`, which depends on `yak_anon_target`. A Cargo package gives its library the dependencies of its binaries, so the library of `app/yak` depends on every crate that only the binary may depend on. The check now treats every target of `app/yak` as the top level.
17. A build script runs in a tree of symbolic links, so `walkdir` in `app/yak_external_cells_bundled/build.rs` needs `follow_links(true)`. The crate compiles in another directory than the one the build script ran in, so its `include_bytes!` paths start from `CARGO_MANIFEST_DIR`.

## Decision Log

- 2026-10-02: `superconsole` becomes a workspace member and loses its own `[workspace]` and lock file (owner: it does not need to publish on its own).
- 2026-10-02: The repository stays on nightly Rust and `cfg(tokio_unstable)` until stable Rust and tokio offer what it uses (owner), so the `cargo` cell passes `[build] rustflags` through (Plan of Work, item 2). The tech-debt tracker lists what stable lacks.
- 2026-10-02: The repository's `prelude` cell reads `prelude/` from the repository instead of the copy bundled in the running binary, and `include` of `cargo_package()` takes targets of other cells, so `yak_external_cells_bundled` names the prelude's files through targets of the `prelude` cell (finding 7). A prelude edit then reaches builds in the repository without a rebuild of the binary, but an edit that the running binary cannot evaluate breaks those builds until the binary is rebuilt. The rejected alternative, a directory argument of `cargo_package()`, keeps the bundled prelude but gives the files no target, so `yak test --changed-since` cannot select the crate after a prelude edit.
- 2026-10-02: The client-only binary of `app/yak` is written by hand in `app/yak/YAK` beside `cargo_package()` (owner). A Cargo package builds with one set of dependencies, so Cargo has no build of a binary without the server crates. The hand-written targets repeat the dependencies of `app/yak/Cargo.toml` without those crates, and a comment in the file says why. The rejected alternative, a separate Cargo package for the client, builds with Cargo too, but it moves `bin/yak.rs` and duplicates the package's manifest.
- 2026-10-02: The `.proto` files reach the build scripts of other protobuf crates through `filegroup` targets in the `YAK` files of their packages and `include` of `cargo_package()`. The build scripts read them by relative path under both Cargo and yak, so the `YAK_PROTO_SRCS` branch of the build scripts is removed.
- 2026-10-02: `pagable_transition_alias` is removed. The workspace dependency on `starlark` enables its `pagable` feature, and the generated targets build with the features that Cargo resolves.
- 2026-10-02: `starlark-rust/starlark/fuzz` stays a separate workspace for `cargo fuzz`, and its `YAK` file is removed. `cargo fuzz` compiles the fuzz target with the coverage instrumentation of libFuzzer, and a build of the target without those flags gives a fuzzer without coverage feedback.
- 2026-10-02: `dice/fuzzy_dice`, `shed/cgroups/use_some_memory`, `shed/completion_verify`, and `starlark-rust/benchmark_memory` become workspace members. `completion_verify` runs `bash`, `fish`, and `zsh` from `PATH`, which removes the RPM downloads of `shed/rpm_download` and the `dnf` requirement. Its unit tests run every shell, so its binary sets `test = false`, and the completion tests of `tests/core/completion` run it.
- 2026-10-02: `toolchains/YAK` keeps its gcc-based C++ toolchain and passes the identities of `rustc` and `go` to the Rust and Go toolchains. The daemon computes no identity for gcc, so the C++ toolchain has none (tech-debt tracker).
- 2026-10-02: The cell applies the `[profile.dev]` table of the workspace's `Cargo.toml`, with its `[profile.dev.package.<name>]` and `build-override` tables, as `cargo build` does (owner, finding 12). The rejected alternative, the flags in `rustc_flags` of `toolchains/YAK`, is smaller but copies the profile by hand and leaves other workspaces at opt-level 0. A setting that selects `[profile.release]` waits for a need.

## Outcomes & Retrospective

The prototype built a working binary, so the generated build is the approach. Items 1 to 6 are done, and the repository builds `//app/yak:yak` from the build files that `yak generate` writes.

## Context and Orientation

- `app/yak_external_cells/src/cargo.rs` runs `cargo metadata` and `rustc --print cfg` for the `cargo` cell, and `app/yak_external_cells_cargo/` generates its packages and the `cargo_package` macro.
- `prelude/rust/cargo_buildscript.bzl` runs build scripts (`buildscript_run`).
- `website/docs/users/languages/rust/cargo.md` documents `yak generate`, `include`, and `test_data`.
- `app/yak_external_cells_bundled/build.rs` embeds the prelude.
- `ARCHITECTURE.md` lists the hand-written targets under "Generated build files".

## Plan of Work

Each item names the finding it resolves. The order puts the cell's general Cargo fidelity first, because other Cargo workspaces need it too.

1. `yak generate` skips the directories of `[project] ignore` (finding 2).
2. The cell passes `[build] rustflags` and `target.<triple>.rustflags` of `.cargo/config.toml` to `rustc --print cfg` and to every crate it builds (finding 10). It does not read `RUSTFLAGS`, which Cargo reads from the environment of each command.
3. A build script runs in a directory with the workspace layout around its package and the files that `include` names, as tests with `run_from_manifest_dir` do (finding 6).
4. `buildscript_run` passes `DEP_<links>_<key>` from the build scripts of `links` packages to the build scripts of their dependents (finding 8).
5. Members compile from paths that give `file!()` the paths of a Cargo build, for example with `--remap-path-prefix` (finding 11).
6. The repository: `superconsole` joins the workspace, `rustls` and `hyper-rustls` drop their default features, the protobuf crates declare their `.proto` files, the root `YAK` keeps its hand-written targets beside `cargo_workspace()`, the prelude reaches `yak_external_cells_bundled` as decided, `toolchains/YAK` uses tool identities, and the 5 crates outside the workspace build (findings 3, 4, 7, 9, 13, 14).
7. The cell applies the `dev` profile of the workspace (finding 12, Decision Log).
8. CI builds `//app/yak:yak` and runs `//app_dep_graph_rules:test_yak_dep_graph`.

## Validation and Acceptance

From the repository root:

- `yak build //app/yak:yak` succeeds.
- `YAK_BINARY=<path of the built binary> tests/.venv/bin/python -m pytest tests -n auto` gives the results of the Cargo-built binary.
- `yak build //app_dep_graph_rules:test_yak_dep_graph` succeeds, and a dependency that `app_dep_graph_rules/rules.bzl` forbids fails it.

## Idempotence and Recovery

`yak generate` rewrites only the files whose contents differ, and `--force` replaces hand-edited `YAK` files, so a run without `--force` keeps the hand-written targets of finding 4.
