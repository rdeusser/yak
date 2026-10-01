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

- A `YAK` file next to each member's `Cargo.toml`, with these three lines:

  ```python
  load("@crates//:workspace.bzl", "cargo_package")

  cargo_package()
  ```

- A `YAK` file at the root of a virtual workspace, which calls
  `cargo_workspace()` instead. It declares the files there that members
  include.

- The `crates` cell in `.yakconfig`, with the `cargo` origin. If the project
  has no `.yakconfig`, the command also writes the `.yakroot`, `.yakconfig`, and
  `toolchains/YAK` files that `yak init` writes.
- `/yak-out` in `.gitignore`, if a `.gitignore` exists and does not ignore it.

The build files list no dependencies, so an edit to a `Cargo.toml` reaches the
next build without another `yak generate`. A new workspace member needs another
run, which writes the member's `YAK` file. A run writes only the files whose
contents differ. It keeps a `YAK` file that differs from the generated one, and
`--force` replaces it. It also writes the build files of the Go modules in the
directory, as [Go modules](../go/modules.md) describes. A directory that holds
both a `Cargo.toml` and a `go.mod` gets one `YAK` file that calls both macros.

## Targets

`cargo_package()` declares the targets of the member in its directory from the
output of `cargo metadata`. They are targets of the member's package, such as
`//crates/server`:

| Cargo target               | yak target                                                                                                                |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| The library                | A `rust_library` named after the package, or `<package>-lib` if a binary has the package's name                           |
| The library's unit tests   | A `rust_test` named `<package>-unittest`                                                                                  |
| Each binary                | A `rust_binary` named after the binary                                                                                    |
| A binary's unit tests      | A `rust_test` named `<package>-unittest` for a binary named after a package without a library, or `<package>-<binary>-unittest` |
| Each integration test      | A `rust_test` named `<package>-<test>`                                                                                    |
| Each example               | A `rust_binary`, or a `rust_library` for a library example, named `<package>-example-<example>`                           |
| The build script           | `<package>-build-script-build`, which compiles it, and `<package>-build-script-run`, which runs it                        |

A label that omits the target name names the target named after the
directory, so `//crates/server` is `//crates/server:server`. If no target has
the directory's name, an alias of that name points to the library, or to the
only binary of a package without a library. The library and the binaries list
the package's tests in `tests`, so `yak test //crates/server` runs the tests
that `cargo test -p server` runs, and `yak test //...` runs the tests that
`cargo test --workspace` runs. Libraries and binaries are visible to every
package, and the other targets only to their own.
A target whose `required-features` are not
all enabled has no yak target, and a target with `test = false` has no test
target.

The tests run as `cargo test` runs them:

- A test runs from a `deps/` directory, and its package's binaries and
  examples sit where Cargo puts them in its `target/debug/` directory. A test
  that looks for an example in the `examples` directory two levels above its
  own binary finds it.
- A test runs in a copy of its package's files and the files it declares, from
  the package's directory in the copy. `CARGO_MANIFEST_DIR` names that
  directory at compile time and at run time, so a test finds its package's
  files through `CARGO_MANIFEST_DIR` or a relative path, as under `cargo test`.
- An integration test gets `CARGO_BIN_EXE_<binary>` for each binary of its
  package.

Each target builds with the features that Cargo resolves for its package. Tests
also get the package's dev-dependencies. A dependency under
`[target.'cfg(...)'.dependencies]` applies on the platforms whose `rustc --print cfg`
output satisfies the condition.

## Files outside a crate's directory

A crate builds with the files of its own package and the files that its Rust
files name in `include!`, `include_str!`, or `include_bytes!`, such as
`include_str!("../../README.md")`. The cell finds those files in a string
literal argument and in `concat!(env!("CARGO_MANIFEST_DIR"), "/path")`. A file
of another member's package or of the workspace's root is exported by that
package's `cargo_package()` or `cargo_workspace()` call, as an `export_file`
target named by its path in the package, visible to the members that include
it. A file of a package with a hand-written build file needs an `export_file`
target of that name there.

Other files reach a crate through targets of their packages. `include` of
`cargo_package()` names the targets whose files the crates read at compile
time, such as files that a build script reads, and `test_data` the targets
whose files the tests read at run time:

```python
# File: crates/server/YAK

load("@crates//:workspace.bzl", "cargo_package")

cargo_package(test_data = ["//crates/fixtures:json"])
```

```python
# File: crates/fixtures/YAK

load("@crates//:workspace.bzl", "cargo_package")

cargo_package()

filegroup(
    name = "json",
    srcs = glob(["data/*.json"]),
    visibility = ["//crates/server:"],
)
```

A target's files appear at their paths in the workspace, so a test of
`crates/server` reads `../fixtures/data/users.json`. The copy that a test runs
in holds only its package's files, its included files, and its `test_data`
files, so a test that reads another file fails. The declarations tell
[`yak test --changed-since`](../../advanced/changed_since.md) which tests a
change to a file can affect. A change to a `test_data` file selects the tests
of the packages that declare it, and a change to an included file also selects
the tests of every package that depends on the crate.

A `[package.metadata.yak]` table in a `Cargo.toml` is an error.

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
- `cargo_package()` declares no targets for benchmarks, and it runs no
  doctests.
- A test that opens an absolute path into the project reads the file whether
  or not it declares it.
- A build file in a directory inside a member, such as one for another
  language, makes that directory a separate package, and the member's globs
  skip its files.
- `yak generate` sets up a Cargo workspace whose directory is the root of the
  yak project. For a workspace in a subdirectory of a project,
  configure the cell by hand, as
  [the `cargo` origin](../../advanced/external_cells.md#the-cargo-origin)
  shows, and write the `YAK` files of the workspace's root and members.
