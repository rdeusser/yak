# Build yak with yak

Follows [the plan contract](../../PLANS.md).

## Purpose

`yak build //app/yak:yak` in this repository builds the yak binary from the `YAK` files that `yak generate` writes from the workspace's `Cargo.toml` files, with the `cargo` cell supplying the third-party crates. The binary passes the integration tests that the Cargo-built binary passes. The repository is the largest test of yak's Rust support: 142 crates, 11 protobuf build scripts, C libraries, and a build script that embeds the prelude.

The yak build of this repository has not loaded since the generator of `third-party/rust/` was removed (tech-debt tracker, "The yak build of this repository has no third-party crates"). The crate dependency check `//app_dep_graph_rules:test_yak_dep_graph` does not run while the build does not load (tech-debt tracker, "The crate dependency rules run only in the Yak build").

The repository owner requires the self-build (2026-10-02).

## Progress

- [x] Prototype: run `yak generate` on a copy of the repository and build `//app/yak:yak`, working around each failure to reach the next (2026-10-02, Surprises & Discoveries).
- [ ] Decide how the prelude reaches `yak_external_cells_bundled` (Decision Log).
- [x] Plan of Work item 1: `yak generate` skips ignored directories (2026-10-02).
- [x] Plan of Work item 2: the `cargo` cell applies the `rustflags` of Cargo's configuration (2026-10-02).
- [ ] Plan of Work items 3 to 8.

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
12. `test_many_rebound_outputs_incremental_rebuild` of `tests/core/build/actions/test_dynamic_output.py` took 612 seconds with the self-built binary and 116 seconds with the Cargo-built one. `yak cquery 'deps(//app/yak:yak)'` took 8.2 and 9.1 seconds, so graph evaluation is not slower. `Cargo.toml` sets `opt-level = 1` for the `dev` profile, which the cell does not apply, so the likely cause is that the self-built binary is unoptimized (unverified). The self-built binary is 510 MB and the Cargo-built one 205 MB.
13. `toolchains/YAK` declares the repository's toolchains by hand, so its actions get no tool identities (`docs/exec-plans/completed/2026-10-01-key-actions-on-their-tools.md`).
14. 5 crates outside the workspace keep hand-written `YAK` files that name `//third-party/rust` targets: `dice/fuzzy_dice`, `shed/cgroups/use_some_memory`, `shed/completion_verify`, `starlark-rust/benchmark_memory`, and `starlark-rust/starlark/fuzz`.

## Decision Log

- 2026-10-02: `superconsole` becomes a workspace member and loses its own `[workspace]` and lock file (owner: it does not need to publish on its own).
- 2026-10-02: The repository stays on nightly Rust and `cfg(tokio_unstable)` until stable Rust and tokio offer what it uses (owner), so the `cargo` cell passes `[build] rustflags` through (Plan of Work, item 2). The tech-debt tracker lists what stable lacks.
- No decision yet on how the prelude reaches `yak_external_cells_bundled` (finding 7).

## Outcomes & Retrospective

The prototype built a working binary, so the generated build is the approach. No milestone is done yet.

## Context and Orientation

- `app/yak_external_cells/src/cargo.rs` runs `cargo metadata` and `rustc --print cfg` for the `cargo` cell, and `app/yak_external_cells_cargo/` generates its packages and the `cargo_package` macro.
- `prelude/rust/cargo_buildscript.bzl` runs build scripts (`buildscript_run`).
- `website/docs/users/languages/rust/cargo.md` documents `yak generate`, `include`, and `test_data`.
- `app/yak_external_cells_bundled/build.rs` embeds the prelude, and `app/yak/bin/yak.rs` selects the allocator with `cfg(yak_build)`.
- `ARCHITECTURE.md` describes the two build definitions under "Two build definitions".

## Plan of Work

Each item names the finding it resolves. The order puts the cell's general Cargo fidelity first, because other Cargo workspaces need it too.

1. `yak generate` skips the directories of `[project] ignore` (finding 2).
2. The cell passes `[build] rustflags` and `target.<triple>.rustflags` of `.cargo/config.toml` (and `RUSTFLAGS`) to `rustc --print cfg` and to every crate it builds (finding 10).
3. A build script runs in a directory with the workspace layout around its package and the files that `include` names, as tests with `run_from_manifest_dir` do (finding 6).
4. `buildscript_run` passes `DEP_<links>_<key>` from the build scripts of `links` packages to the build scripts of their dependents (finding 8).
5. Members compile from paths that give `file!()` the paths of a Cargo build, for example with `--remap-path-prefix` (finding 11).
6. The repository: `superconsole` joins the workspace, `rustls` and `hyper-rustls` drop their default features, the protobuf crates declare their `.proto` files, the root `YAK` keeps its hand-written targets beside `cargo_workspace()`, the prelude reaches `yak_external_cells_bundled` as decided, `toolchains/YAK` uses tool identities, and the 5 crates outside the workspace build (findings 3, 4, 7, 9, 13, 14).
7. Confirm the cause of finding 12, and decide whether the cell applies the `opt-level` of Cargo's profiles.
8. CI builds `//app/yak:yak` and runs `//app_dep_graph_rules:test_yak_dep_graph`.

## Validation and Acceptance

From the repository root:

- `yak build //app/yak:yak` succeeds.
- `YAK_BINARY=<path of the built binary> tests/.venv/bin/python -m pytest tests -n auto` gives the results of the Cargo-built binary.
- `yak build //app_dep_graph_rules:test_yak_dep_graph` succeeds, and a dependency that `app_dep_graph_rules/rules.bzl` forbids fails it.

## Idempotence and Recovery

`yak generate` rewrites only the files whose contents differ, and `--force` replaces hand-edited `YAK` files, so a run without `--force` keeps the hand-written targets of finding 4.
