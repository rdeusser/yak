---
id: changed_since
title: Testing What Changed
---

`yak test --changed-since <revision>` runs the tests that the changes since a
Git revision can affect and skips the rest. The revision is anything Git
resolves to a commit, such as `main`, `origin/main`, a branch, a tag, or a
commit ID.

```sh
yak test --changed-since origin/main //...
```

## The changes

The changes are the files that differ between the working tree and the merge
base of the revision and `HEAD`, the commit that `git diff <revision>...` starts
from. Uncommitted changes and untracked files count. Files that Git ignores and
files under `yak-out` do not count. Commits that reach the revision after the
branch point do not count, so the selection stays the same as the revision moves
on.

## What a change affects

yak reads the current target graph and decides for each package whether its
evaluation can differ from the merge base. A package can differ when:

- its build file changed, appeared, or disappeared,
- a `PACKAGE` file in its directory or above it changed,
- a file that it loads changed, or a file that such a file loads,
- a file appeared in its directory or disappeared from it, which can change what
  a `glob` returns,
- it belongs to a cargo cell and a file that `cargo metadata` reads changed,
  such as a `Cargo.toml`, `Cargo.lock`, or `.cargo/config.toml`, or a file whose
  presence makes a Cargo target, such as `src/lib.rs` or a file in `tests/`.

A target changed when its package can differ or one of its inputs changed. A
target is affected when it changed or depends on a target that changed. The
dependencies include execution, toolchain, and configuration dependencies, the
target platform, and the execution platforms. yak tests each matched target that
is affected, and each matched target whose `tests` attribute names an affected
target.

yak tests every matched target when:

- the configuration of a cell at the merge base differs from its current
  configuration,
- a submodule changed,
- a package that defines a configuration target, such as a constraint or a
  platform, can differ, because transitions and modifiers refer to configuration
  targets without depending on them,
- `rust-toolchain` or `rust-toolchain.toml` in the project root changed, because
  rustup picks the compiler of every Rust action from it,
- a path that `[test] changed_since_select_all` names changed.

`yak test` prints how many matched targets it tests, or why it tests all of
them.

## Undeclared inputs

The selection is only as complete as the inputs that targets declare. A test
that reads a file it does not declare can break without being selected. The
tests of a Cargo workspace member run in a copy of the files they declare, so a
read of another file fails (see [Cargo workspaces](../languages/rust/cargo.md)).

`[test] changed_since_select_all` lists paths, relative to the project root,
whose changes select every test. It is for files that affect many targets
without being their inputs, such as a script that a toolchain runs:

```ini
[test]
  changed_since_select_all = tools/lint, .clang-tidy
```

A change outside the repository, such as a new compiler on `PATH`, is not a
change.

## Precision

When a package can differ, each of its targets counts as changed. A Cargo
workspace that `yak generate` sets up defines all of its targets in the root
package, so a change to a `Cargo.toml`, or a file that appears or disappears in
the workspace, selects every test of the workspace. An edit to an existing
source file selects the targets that have it as an input and their dependents.
