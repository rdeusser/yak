---
id: cargo
title: Cargo workspaces
---

# Cargo workspaces

`yak generate` sets up a Cargo workspace to build with yak. The workspace's
`Cargo.toml` files and `Cargo.lock` stay the source of truth, and `cargo build`
keeps working.

## Generating the build files

Run `yak generate` in the directory of the workspace's root `Cargo.toml`, then
build:

```sh
yak generate
yak build //...
```

`yak generate` writes these files:

- A `YAK` file in the directory of each workspace member, with these three
  lines:

  ```python
  load("@crates//:workspace.bzl", "cargo_workspace_member")

  cargo_workspace_member()
  ```

- The `crates` cell in `.yakconfig`, with the `cargo` origin. If the project
  has no `.yakconfig`, the command also writes the `.yakroot`, `.yakconfig`, and
  `toolchains/YAK` files that `yak init` writes.
- `/yak-out` in `.gitignore`, if a `.gitignore` exists and does not ignore it.

The build files list no dependencies, so an edit to a `Cargo.toml` reaches the
next build without another `yak generate`. Run it again after adding a
workspace member. A run writes only the files whose contents differ. It keeps a
member's `YAK` file that differs from the generated one, and `--force` replaces
it.

## Targets

`cargo_workspace_member()` declares the targets of the member in its directory,
from the output of `cargo metadata`:

| Cargo target               | yak target                                                                                              |
| -------------------------- | ------------------------------------------------------------------------------------------------------- |
| The library                | A `rust_library` named after the package, or `<package>-lib` if a binary has the package's name         |
| The library's unit tests   | A `rust_test` named `<library>-unittest`                                                                |
| Each binary                | A `rust_binary` named after the binary                                                                  |
| Each integration test      | A `rust_test` named after the test                                                                      |
| The build script           | `<package>-build-script-build`, which compiles it, and `<package>-build-script-run`, which runs it      |

For example, `yak run //crates/app:app` runs the binary `app` of the member in
`crates/app`, and `yak test //...` runs every test of the workspace.

Each target builds with the features that Cargo resolves for its package. Tests
also get the package's dev-dependencies. A dependency under
`[target.'cfg(...)'.dependencies]` applies on the platforms whose `rustc --print cfg`
output satisfies the condition.

## Third-party crates

The `crates` cell holds a target for each crates.io package in the dependency
graph, named `<name>-<version>`, such as `crates//:serde-1.0.228`. A crate with
one version in the graph also has an alias named after the crate, such as
`crates//:serde`. Each crate downloads from crates.io in a build action, which
checks it against the checksum in `Cargo.lock`.

The daemon generates the cell's files in memory, so no third-party target is
checked in. It runs `cargo metadata --locked` when a `Cargo.toml` or
`Cargo.lock` of the workspace changes, and `rustc --print cfg --target <triple>`
for each platform. The machine that runs the daemon therefore needs `cargo` and
`rustc`. `Cargo.lock` must be up to date, which `cargo build` or
`cargo update --workspace` ensures after a dependency changes.

## Limitations

- The cell builds third-party packages from crates.io only. A dependency from
  Git, from another registry, or from a path outside the workspace is an error.
- `cargo metadata` reports one feature set per package. Cargo can build a
  package that is both a build dependency and a normal dependency with a
  different feature set for each, and yak builds it with the union of both.
- Platform-specific dependencies resolve for the Cargo platforms of
  `prelude//rust/cargo_package.bzl`: `linux-arm64`, `linux-riscv64`,
  `linux-x86_64`, `macos-arm64`, `macos-x86_64`, `wasi`, `wasm32`,
  `windows-gnu`, and `windows-msvc`.
- A crate builds with the files of its own directory. A crate that reads a file
  outside it, such as `include_str!("../../README.md")`, fails to compile.
- `yak generate` runs in the root of a Cargo workspace whose directory is also
  the root of the yak project. For a workspace in a subdirectory of a project,
  configure the cell by hand, as
  [the `cargo` origin](../../advanced/external_cells.md#the-cargo-origin)
  shows, and write the members' `YAK` files.
