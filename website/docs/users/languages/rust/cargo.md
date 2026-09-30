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

The `crates` cell holds a package for each third-party package in the
dependency graph, in a directory named `<name>-<version>`, such as
`crates//serde-1.0.229`. A package that does not come from crates.io, such as
one from Git or a private registry, has a hash of its source appended, such as
`crates//mylib-0.3.0-1a2b3c4d`, so that another commit or registry gets another
directory. The package's library is named after the package, such
as `crates//serde-1.0.229:serde`. The root of the cell has an alias named
`<name>-<version>` for each package, and an alias named after the package when
it has one version in the graph, such as `crates//:serde`.

The daemon runs `cargo metadata --locked` when a `Cargo.toml` or `Cargo.lock`
of the workspace changes, and `rustc --print cfg --target <triple>` for each
platform. `cargo metadata` downloads each package that is not in Cargo's cache
and checks it against `Cargo.lock`. It follows the workspace's
`.cargo/config.toml` and Cargo's credentials, so a package from crates.io, from
a private registry, from a mirror that source replacement names, from a
`vendor/` directory, or from Git builds as it does with `cargo build`. The first
build that reads a package's files copies them from where Cargo put them into
`yak-out`. Build actions then need no network access.

The daemon generates the build files in memory, so no third-party target is
checked in. The machine that runs the daemon needs `cargo`, `rustc`, and the
network access and credentials that `cargo build` needs there. The daemon keeps
the environment it started with, so run `yak kill` after changing an
environment variable that Cargo reads, such as a registry token. `Cargo.lock`
must be up to date, which `cargo build` or `cargo update --workspace` ensures
after a dependency changes.

## Limitations

- A path dependency outside the workspace is an error, because the cell
  treats a package's sources as unchanging. Add the package to the workspace's
  `members`.
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
