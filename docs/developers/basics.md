# Developer Basics

This file is `docs/developers/basics.md`. It is required reading for working on the code in this repository.

This file is optimized for both humans and LLMs and must be kept short. Detailed explanations belong in adjacent files in this directory or other documentation from which humans or LLMs can pull as needed. `ARCHITECTURE.md` at the repository root maps the crates.

## Building

The commands in this file run from the repository root, except where a block changes directory. `rust-toolchain.toml` pins a nightly toolchain, which `rustup` installs on first use.

```bash
# Build the debug binary at target/debug/yak
cargo build --bin=yak
```

Cargo builds compile the protobuf definitions with the `protoc` binary from the `protoc-bin-vendored` crate. Set `YAK_BUILD_PROTOC` and `YAK_BUILD_PROTOC_INCLUDE` to use another `protoc`, which NixOS requires. `flake.nix` sets both in its development shell.

To try a change, run the built binary in a test project such as `examples/no_prelude`. Pass `--isolation-dir` with a name of your own so the binary starts its own daemon and leaves any other daemon for that project running:

```bash
cd examples/no_prelude
../../target/debug/yak --isolation-dir dev build //rust:main
../../target/debug/yak --isolation-dir dev kill
```

The repository can also build itself with yak. The yak build loads the prelude bundled in the `yak` binary (`[external_cells] prelude = bundled` in `.yakconfig`), so it needs a binary built from this repository. `bootstrap/reindeer` generates the yak rules for the third-party crates and needs `dotslash` on `PATH` (see `website/docs/about/bootstrapping.md`):

```bash
cargo build --bin=yak
./bootstrap/reindeer --third-party-dir third-party/rust buckify
target/debug/yak build //:yak
```

`reindeer` writes `third-party/rust/YAK` and `third-party/rust/Cargo.lock`, and Git ignores both. Until `third-party/rust/YAK` exists, any command that loads the `third-party/rust` package fails with the `reindeer` command to run. The yak build succeeds on Linux but fails on macOS, as [the tech-debt tracker](../exec-plans/tech-debt-tracker.md) records.

On Windows, the build uses clang-cl when `-c cxx.windows_compiler_type=clang` is on the command line. The `toolchains` cell has no `.yakconfig` of its own, so the setting has no effect in the repository's `.yakconfig`.

## Validation

```bash
# clippy with warnings denied, rustdoc with warnings denied, then unit tests and doc tests
python3 test.py
# The same for the named packages only
python3 test.py buck2_core buck2_common
# One stage only
python3 test.py --lint-only buck2_core
python3 test.py --rustdoc-only buck2_core
python3 test.py --test-only buck2_core
# Format
cargo fmt --all
```

CI (`.github/workflows/build-and-test.yml`) runs `cargo build --bin=yak` and then `python3 test.py --ci` on Linux, macOS, and Windows. With `--ci`, `test.py` also fails when the run leaves changes in the Git working tree. CI does not check formatting.

Clippy's lint levels live in `[workspace.lints]` in `Cargo.toml`, and `clippy.toml` bans panicking datetime and duration APIs. Plain `cargo clippy` applies both, and `test.py` adds `--deny=warnings`. Eight crates copy the whole lint table into their own `Cargo.toml` to add a `check-cfg` entry (`app/buck2`, `app/buck2_daemon`, `allocative/allocative`, `shed/mini_vec`, and `starlark`, `starlark_syntax`, `starlark_map`, and `starlark_lsp` under `starlark-rust/`), so a change to a lint level updates those copies too.

Unit tests live next to the code they test. Crates named `*_tests` (for example `app/buck2_build_api_tests`) hold tests that need late bindings from several crates.

Golden tests compare output with checked-in files whose names contain `.golden`. To regenerate them, rerun the test with the regeneration variable set:

- `YAK_RUST_REGENERATE_GOLDEN_TESTS=1` for tests that use `buck2_util::golden_test_helper`.
- `STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1` for `starlark-rust/`.
- `ALLOCATIVE_REGENERATE_TESTS=1` for `allocative/`.
- `YAK_UPDATE_GOLDEN=1` for the integration tests under `tests/`. The update accepts whatever the binary prints, so review each golden file diff.

The integration tests under `tests/` run `target/debug/yak` against small projects with pytest. `tests/README.md` gives the commands, the markers that skip tests that need Remote Execution, cgroups, or helper programs, and the golden file workflow. `tests/core/README.md` gives the guidelines for writing them.

## Coding conventions

Most important of all: Most questions can be answered by matching the conventions and style of nearby code.

Standard `rustfmt` conventions apply, with the options in `rustfmt.toml`. Beyond that:

- **HashMaps**: use `buck2_hash::BuckMutMap`, not `fxhash::FxHashMap`.
- **Cloning**: prefer `.dupe()` over `.clone()` for types that implement `Dupe`
  (e.g. `Arc`-wrapped types). Use `gazebo` utilities — particularly `dupe` —
  where they fit.
- **String conversion**: prefer `.to_owned()` over `.to_string()` for `&str` →
  `String`.
- **Durations**: never call `.elapsed()`; compute `Instant::now() - start` so the
  time source is explicit. No lint enforces this in this repository.
- **Imports**: use `use crate::foo::bar`, not `use super::bar`. Place all `use`
  statements at the module level — never inside a function or block. Test
  modules may use `use super::*;` at the top.
- **Modules**: a module should contain either submodules OR types/functions, not
  both.
- **PartialEq/Hash with ignored fields**: use the `derivative` crate.

[.llms/code_quality.md](./.llms/code_quality.md) sets the expectations for leaving code better than you found it, and [.llms/writing.md](./.llms/writing.md) covers comments and documentation.

### Error message style

- Names (variables, targets, files, ...) should be quoted with backticks, e.g.
  ``Variable `x` not defined``.
- Lists should use square brackets, e.g. ``Available targets: [`aa`, `bb`]``.
- Error messages should start with an upper case letter and should not end with
  a period.

## Error handling

yak uses `buck2_error` replacing both `anyhow` and `thiserror`. The must-knows:

- Return `buck2_error::Result<T>`.
- Define error types with `#[derive(Debug, buck2_error::Error)]` and tag them
  with `#[buck2(tag = ...)]` (no `thiserror::Error`).
- Use the `buck2_error!` macro for ad-hoc errors.
- `.expect()`, `.unwrap()`, etc. are ok for file-local invariant violations/"this should never
  happen" cases. If not file-local, prefer `internal_error!()`, `.internal_error("...")?` or
  `.with_internal_error(|| ...)` if possible.
- Inspecting or creating `buck2_error::Error`s in non-error codepaths is strongly discouraged.
  Represent states that are not errors using types that are not errors or at least dedicated,
  semantically clear error types.

For more details including about defining errors, tagging, conversion, and context see [Error
Handling](./error_handling.md).

## Feature gates

Gate new or risky behavior with yakconfig, not environment variables: a
`[yak]`-section key, read where DICE can track it (grep for
`BuckconfigKeyRef` with `section: "yak"` for the pattern), and named so
the value flips false -> true as the feature rolls out. Use
`RolloutPercentage` in place of `bool` when you want hostname-hashed
percentage rollout. Reserve `buck2_env!` for the few places configuration
cannot reach: the client before it talks to the daemon, and test-only
knobs.

## Porting changes from upstream

`facebook/buck2` builds inside Meta's internal repository, and its code marks what only that build uses (`#[cfg(fbcode_build)]` branches, `@oss-disable` and `@oss-enable` comments, `is_open_source()` checks, and `fbcode//` or `fbsource//` labels). This repository has none of these markers. A change ported from upstream keeps the open-source side of each marker and drops the rest.

Upstream `BUCK` files load macros from Meta's cells and name crates by their path inside Meta's repository. A ported build file is named `YAK`, loads `//build_defs:rust.bzl` or `//build_defs:proto.bzl`, names third-party crates `//third-party/rust:<crate>` in place of `fbsource//third-party/rust:<crate>`, and names crates of this repository `//<path>:<crate>` in place of `//buck2/<path>:<crate>`.

## Rust dependencies

Each crate has a `Cargo.toml` and a `YAK` file, and a dependency change updates both:

1. Add the version to `[workspace.dependencies]` in the root `Cargo.toml` if it is new, and name it in the crate's `Cargo.toml` with `workspace = true`.
2. Add the same dependency to the crate's `YAK` file. Third-party crates are named `//third-party/rust:<crate>`, and crates in this repository are named `//<path>:<crate>` (for example `//app/buck2_core:buck2_core`).
3. For a new third-party crate, also add it to `third-party/rust/Cargo.toml`, which the yak build reads through `reindeer`. A crate with a build script, or one that reads Cargo environment variables at compile time, also needs `third-party/rust/fixups/<crate>/fixups.toml`. `reindeer buckify` fails when a fixup configures a build script that the resolved crate versions no longer have, so a dependency change that drops or upgrades a crate can require editing or deleting its fixup.
4. Check the dependency against the crate dependency rules in `ARCHITECTURE.md`. `target/debug/yak build //app_dep_graph_rules:test_buck2_dep_graph` fails when a dependency breaks one.

## Debugging and performance

- [debugging.md](./debugging.md) covers logs, event logs, and tracing.
- [what-ran.md](./what-ran.md) shows how to find and rerun the commands a build ran.
- [perf/basics.md](./perf/basics.md) is the starting point for profiling and benchmarking.
