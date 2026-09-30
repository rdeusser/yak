# Generate YAK files from Cargo and Go projects

Follows [the plan contract](../../PLANS.md).

## Purpose

A Rust or Go project that builds with `cargo build` or `go build` can be built with yak after one `yak generate`.
Its `Cargo.toml`, `Cargo.lock`, `go.mod`, and `go.sum` stay the source of truth, and the project still builds without yak.
yak builds every dependency that Cargo or Go would build, including third-party crates and modules.

The checked-in output stays small. The repository owner's requirement (2026-09-29) is a few hundred lines at most for a whole project. Reindeer wrote a 12,000-line build file for Roost, a Rust project of the owner's with 6 workspace crates and 775 packages in its `Cargo.lock`. The number of checked-in lines grows with the project's own crates and packages, and never with its third-party dependencies.

To see it working, run `yak generate` in a Cargo workspace and a Go module, count the lines it writes, and run `yak build //...` in each.

## Progress

- [x] The owner chooses the direction: the Cargo and Go files stay the source of truth, and yak derives its build files from them (2026-09-29).
- [x] The owner chooses to resolve dependencies by running `cargo` and `go` (2026-09-29).
- [x] Milestone 1, prototype: a `YAK` file builds a crate from its own `Cargo.toml` through `load()` (2026-09-30). The approach is kept.
- [x] Milestone 2, prototype: prelude rules build third-party crates from crates.io that `cargo metadata` describes, including a proc macro and a build script (2026-09-30). The approach is kept for one platform. Resolving several platforms remains for milestone 3.
- [ ] Milestone 3: the `cargo` external cell origin.
- [ ] Milestone 4: the prelude macro for first-party crates.
- [ ] Milestone 5: `yak generate` for Cargo workspaces.
- [ ] Milestone 6: the `go` external cell origin, the first-party Go rules, and `yak generate` for Go modules.
- [ ] Milestone 7: documentation, and validation against Roost and the example projects.

## Surprises & Discoveries

- Milestone 1 (2026-09-30): a `YAK` file of five lines builds a crate from its `Cargo.toml`. The project had a workspace `Cargo.toml` with `[workspace.package] edition`, a library crate `util`, and a crate `hello` with a library, a binary, and `util = { path = "../util" }`. Each crate's `YAK` file was:

  ```python
  load(":Cargo.toml", manifest = "value")
  load("//:Cargo.toml", workspace = "value")
  load("//:cargo.bzl", "cargo_package")

  cargo_package(manifest, workspace)
  ```

  `cargo_package` (46 lines) took the name, the edition (inherited through `edition.workspace = true`), and the path dependencies from the manifest, and globbed `src/**/*.rs`. `yak run //hello:hello-bin` and `cargo run -p hello` both printed `HELLO FROM UTIL`. After the dependency on `util` was removed from `hello/Cargo.toml`, the next `yak build //hello:hello-bin` failed with ``error[E0433]: cannot find module or crate `util` in this scope``, and restoring it made the build pass. An edit to `Cargo.toml` reaches the build without regenerating the `YAK` file.
- Milestone 2 (2026-09-30): third-party crates build from `cargo metadata` output with the prelude's rules. The project depended on `serde` with `derive`, `serde_json`, `libc`, and `memchr` under `[target.'cfg(unix)'.dependencies]`, which resolve to 13 packages. A script read `cargo metadata --locked --format-version 1 --filter-platform aarch64-apple-darwin` and the checksums in `Cargo.lock`, and wrote per crate an `http_archive` of `https://static.crates.io/crates/<name>/<name>-<version>.crate`, a `cargo.rust_library` (with `proc_macro` for `serde_derive`), and for crates with a build script a `cargo.rust_binary` and a `buildscript_run`. The library took the build script's results through `env = {"OUT_DIR": "$(location :<id>-build-script-run[out_dir])"}` and `rustc_flags = ["@$(location :<id>-build-script-run[rustc_flags])"]`. `yak run //app:app` printed `{"pid":7,"found":2}`, as `cargo run -p app` did.
- `buildscript_run` sets only `CARGO_PKG_NAME` and `CARGO_PKG_VERSION` (`prelude/rust/cargo_buildscript.bzl`). `serde_core`'s build script failed with `called Result::unwrap() on an Err value: NotPresent` because it reads `CARGO_PKG_VERSION_PATCH`. Cargo also sets the `CARGO_PKG_VERSION_MAJOR`, `_MINOR`, `_PATCH`, and `_PRE` parts, which the rule can derive from `version`, and package metadata such as `CARGO_PKG_AUTHORS`, which comes from the manifest. The prototype passed them through `env`.
- `cargo metadata` reports one feature set per package (`resolve.nodes[].features`). Cargo's version 2 resolver can build a package with different features for build dependencies and proc macros than for normal dependencies, so this feature set can be a superset of what `cargo build` uses for one of them. The prototype's crates did not exercise the difference.

## Decision Log

- 2026-09-29: The project's `Cargo.toml`, `Cargo.lock`, `go.mod`, and `go.sum` are the source of truth (owner). Autocargo, which Meta released at `github.com/facebookexperimental/autocargo` and archived on 2025-06-05, converts build files to `Cargo.toml`, the opposite direction.
- 2026-09-29: No third-party target is written into the project's repository. A checked-in line per third-party package would already exceed the limit for Roost (775 packages). Third-party targets live in external cells whose build files the daemon creates in memory, as the bundled prelude cell does.
- 2026-09-29: The daemon resolves the dependency graph by running `cargo metadata` and `go list` (owner). Their output matches what `cargo build` and `go build` use, which a reimplementation of Cargo's feature resolution would not guarantee. The Go rules already run the `go` binary. The Rust rules run `rustc`, and a Rust installation through rustup includes `cargo`. The resolution runs on the machine that runs the daemon, so that machine needs `cargo` or `go`, and it may need network access to fetch manifests that are not cached.
- 2026-09-29: Third-party sources download in build actions through `http_archive`, verified by the checksums in `Cargo.lock` and `go.sum`. The daemon only writes the rules.

## Outcomes & Retrospective

Nothing yet.

## Context and Orientation

Terms:

- A first-party crate or package is one in the project's own repository. A third-party crate or module comes from a registry, such as crates.io or the Go module proxy.
- An external cell is a cell whose files do not come from the project's repository. `[external_cells]` in the root `.yakconfig` names each one and its origin. `website/docs/users/advanced/external_cells.md` describes them for users.

External cells:

- `app/yak_core/src/cells/external.rs` defines `ExternalCellOrigin`, which has the `Bundled` and `Git` origins.
- `app/yak_common/src/legacy_configs/cells.rs` parses `[external_cells]` and the `[external_cell_<name>]` sections (`parse_external_cell_origin`).
- `app/yak_common/src/external_cells.rs` defines `ExternalCellsImpl`, and `app/yak_common/src/file_ops/delegate.rs` defines `FileOpsDelegate`, the interface through which the interpreter reads a cell's files. `FileOpsKey::compute` in the same file picks the delegate for an external cell.
- `app/yak_external_cells/src/lib.rs` dispatches on the origin. `app/yak_external_cells/src/bundled.rs` serves an in-memory file tree and declares its files to the materializer, which is the model for a generated cell. `app/yak_external_cells/src/git.rs` is the model for an origin that runs an outside program.
- `app/yak_core/src/fs/yak_out_path.rs` (`resolve_external_cell_source`) places an external cell's sources under `yak-out/v2/external_cells/<origin>/`, and each new origin extends its match.

Loading data:

- `load()` of a file whose name ends in `.toml` or `.json` imports its contents as `value` (`app/yak_interpreter/src/load_module.rs`, `website/docs/users/loading_data.md`). The file is read through DICE, so edits to it invalidate the files that load it. `Cargo.toml` can be loaded this way. `go.mod`, `go.sum`, and `Cargo.lock` cannot, because of their extensions.

Prelude rules:

- `prelude/rust/cargo_package.bzl` wraps `rust_library` and `rust_binary` for third-party crates and maps per-platform attributes to `select()`.
- `prelude/rust/cargo_buildscript.bzl` runs a crate's build script (`buildscript_run`) and exposes its `OUT_DIR` and `rustc` flags.
- `prelude/http_archive/http_archive.bzl` downloads and unpacks an archive checked by `sha256`. `.crate` files unpack as `tar.gz`.
- `prelude/go/tools/gopackagesdriver/third-party/YAK` is a hand-written example of third-party Go modules: an `http_archive` of the module zip from `proxy.golang.org` with a sub-target per package directory, and a `go_library` per package.
- `prelude/go/package_builder.bzl` picks the files that match the target platform in the build action, so a Go package's file list does not depend on the platform.

Commands:

- `app/yak/src/lib.rs` registers the subcommands. `app/yak_client/src/commands/init.rs` is a client-side command that writes project files, which `yak generate` can follow.

## Plan of Work

Milestones 1 and 2 are prototypes. Each tests an assumption that later milestones depend on.

1. Prototype: first-party crates from `Cargo.toml`. In a scratch project with one crate, a `YAK` file loads `Cargo.toml` and a macro in a local `.bzl` file declares a `rust_library` and a `rust_binary` from it. Keep the approach if `yak build //...` builds the crate and an edit to `Cargo.toml` changes the next build. Otherwise the generator writes each crate's dependencies into its `YAK` file.
2. Prototype: third-party crates from `cargo metadata`. A script turns the `cargo metadata --locked --format-version 1` output of a small project into `http_archive`, `rust_library`, and `buildscript_run` targets. The project depends on `serde` with `derive` (a proc macro), `libc` (a build script), and one crate with a platform-specific dependency. Keep the approach if `yak build //...` builds and the binary runs. Record which metadata `cargo metadata` lacks, such as the separate host and target features of Cargo's version 2 resolver.
3. The `cargo` origin. `ExternalCellOrigin` gains a `Cargo` origin that names the project's `Cargo.toml`. The origin reads `Cargo.toml` and `Cargo.lock` through DICE, runs `cargo metadata`, and serves one package per third-party crate, written the way milestone 2 found to work. The cell's files are cached under a DICE key of the manifest and lock file contents.
4. A `cargo_package` macro in `prelude/rust/` declares a first-party crate's targets from its loaded `Cargo.toml` (library, binaries, tests, build script, features, dependencies, target-specific dependencies, and `workspace = true` inheritance). Dependencies on third-party crates name targets in the `cargo` cell.
5. `yak generate` in `app/yak_client/src/commands/generate.rs`. In a Cargo workspace it writes a `YAK` file per crate, the `[external_cells]` entry, and the `.yakconfig` it needs. It writes a file only when the content differs and leaves hand-written `YAK` files alone unless asked to replace them.
6. Go. A `go` origin runs `go list -m -json all` and `go list -deps -json` and serves a package per third-party Go package, downloaded from the module proxy and checked against `go.sum`. First-party Go packages need their imports listed, because `load()` cannot read `.go` files, so `yak generate` writes a `go_library`, `go_binary`, or `go_test` per first-party package with its dependencies.
7. Documentation in `website/docs/` for `yak generate` and the two origins, `ARCHITECTURE.md` for the new crates and origins, and `CHANGELOG.md`.

## Validation and Acceptance

- Milestones 1 and 2 record their commands and results in Surprises & Discoveries.
- From the repository root, `python3 test.py` passes for every changed package, and new unit tests cover the origin's translation of `cargo metadata` and `go list` output.
- Integration tests under `tests/` run `yak generate` against small Cargo and Go projects, build them with yak, and compare the programs' output with the output of `cargo run` and `go run`.
- In Roost, `yak generate` writes fewer than 300 lines in total (`git diff --stat` after the command), and `yak build //...` succeeds with the same targets that `cargo build --workspace` builds.

## Idempotence and Recovery

`yak generate` can run again at any time, and a run with unchanged inputs writes nothing. Generated cells live in memory and under `yak-out/`, so `yak clean` removes them and the next command recreates them.
