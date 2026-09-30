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

- A `YAK` file at the root of the workspace, with these three lines:

  ```python
  load("@crates//:workspace.bzl", "cargo_workspace")

  cargo_workspace()
  ```

- The `crates` cell in `.yakconfig`, with the `cargo` origin. If the project
  has no `.yakconfig`, the command also writes the `.yakroot`, `.yakconfig`, and
  `toolchains/YAK` files that `yak init` writes.
- `/yak-out` in `.gitignore`, if a `.gitignore` exists and does not ignore it.

The build file lists no dependencies, so an edit to a `Cargo.toml` reaches the
next build without another `yak generate`, and so does a new workspace member.
A run writes only the files whose contents differ. It keeps a `YAK` file at the
root that differs from the generated one, and `--force` replaces it. It stops
with an error if a member's directory has a `YAK` file of its own, because that
file would make the member a separate package.

## Targets

`cargo_workspace()` declares the targets of every workspace member from the
output of `cargo metadata`. All of them are targets of the package at the root
of the workspace:

| Cargo target               | yak target                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ |
| The library                | A `rust_library` named after the package, or `<package>-lib` if a binary has the package's name              |
| The library's unit tests   | A `rust_test` named `<package>-unittest`                                                                     |
| Each binary                | A `rust_binary` named after the binary, or `<package>-<binary>` if two members have binaries of that name    |
| Each integration test      | A `rust_test` named `<package>-<test>`                                                                       |
| The build script           | `<package>-build-script-build`, which compiles it, and `<package>-build-script-run`, which runs it           |

For example, `yak run //:app` runs the binary `app`, and `yak test //...` runs
every test of the workspace.

Each target builds with the features that Cargo resolves for its package. Tests
also get the package's dev-dependencies. A dependency under
`[target.'cfg(...)'.dependencies]` applies on the platforms whose `rustc --print cfg`
output satisfies the condition.

## Files outside a crate's directory

A crate builds with the files of its own directory. A crate that also reads
files elsewhere in the workspace, such as `include_str!("../../README.md")`,
lists them in its `Cargo.toml`, relative to its directory:

```toml
[package.metadata.yak]
include = ["../../README.md", "../../assets/*.json"]
```

Cargo ignores the `[package.metadata]` table. Each entry is a glob pattern, and
it must name files inside the workspace.

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

A build script can link a library that it builds into its `OUT_DIR` or finds
on the machine, such as through pkg-config. The links of the crates that
depend on the package get the library's directory, as with Cargo. yak copies
the shared libraries of an `OUT_DIR` into the build script's `shared_libs`
output, and a binary that links them finds them there through an rpath
relative to the binary. Cargo puts those directories on the dynamic library
path of the programs it runs.

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
- `cargo_workspace()` declares no targets for examples or benchmarks. A test
  that looks for an example in Cargo's `target` directory fails.
- A build file in a directory inside the workspace, such as one for another
  language, makes that directory a separate package, and the members' globs
  skip its files.
- `yak generate` runs in the root of a Cargo workspace whose directory is also
  the root of the yak project. For a workspace in a subdirectory of a project,
  configure the cell by hand, as
  [the `cargo` origin](../../advanced/external_cells.md#the-cargo-origin)
  shows, and write the `YAK` file at the root of the workspace.
