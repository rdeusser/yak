---
id: modules
title: Go modules
---

# Go modules

A [`go` external cell](../../advanced/external_cells.md#the-go-origin) declares
the targets of a Go module from its `go.mod` and its sources. `go.mod`, `go.sum`,
and the imports of the module's files stay the source of truth, and `go build`
keeps working.

## Generating the build files

Run `yak generate` in the directory that holds the project's Go modules, then
build:

```sh
yak generate
yak build //...
```

`yak generate` finds each `go.mod` below the directory, skipping the
directories that `go` skips in `./...` (names that start with `.` or `_`, and
`testdata`) and `vendor`. It writes these files:

- A `YAK` file next to each `go.mod`, with these three lines:

  ```python
  load("@gomod//:module.bzl", "go_module")

  go_module()
  ```

- A go cell for each module in `.yakconfig`. The module at the root of the
  project has the cell `gomod`, and the module in `services/api` has the cell
  `gomod_services_api`. If the project has no `.yakconfig`, the command also
  writes the `.yakroot`, `.yakconfig`, and `toolchains/YAK` files that
  `yak init` writes.
- `/yak-out` in `.gitignore`, if a `.gitignore` exists and does not ignore it.

A run writes only the files whose contents differ. It keeps a `YAK` file that
differs from the generated one, and `--force` replaces it. In a directory that
also holds a `Cargo.toml`, the build file calls the Cargo macro too.

The cell runs `go list` for macOS, Linux, and Windows on `amd64` and `arm64`,
and each target's dependencies select on the prelude's `os` and `cpu`
constraints where those platforms differ. Third-party packages build from the
versions that `go.mod` selects, checked against `go.sum`. A new import, a new
file, or a change to `go.mod` reaches the next build without a generator step.
The cell runs `go list` again only when the names of the module's files, the
build constraints, package clauses, imports, or `//go:embed` lines of its Go
files, `go.mod`, `go.sum`, or the output of `go version` change.

## The targets

`go_module()` declares targets for each package of the module:

| Package | Targets |
| --- | --- |
| A library | A `go_library` named by its directory relative to the build file, and a `go_test` named `<library>-test` when it has tests. The test also builds the package's external tests (`package <name>_test`). |
| `package main` | A `go_binary` named by the last element of its directory, as `go build` names the binary, and a `go_test` named `<binary>-test` when it has tests. |
| The package in the build file's directory | Targets named by the last element of the directory, or of the module path for the module's root. |

Two binaries with the same name take their directories as names instead, such
as `cmd/a/server` and `cmd/b/server`. Any other name that two packages share
is an error. In a module with the packages `cmd/app` and `greet`,
`yak run //:app` runs the binary and `yak test //...` runs the tests of `greet`.

## Test data

A test runs in its package's directory, as with `go test`, and reads the files
of that directory and of the directories below it, apart from the directories
of other Go packages. A test that reads `testdata/input.json` or
`fixtures/repo.git` needs no declaration. A test that reads files elsewhere
names them in `test_data`, which maps the directory of a package, relative to
the build file, to glob patterns relative to the build file:

```python
load("@gomod//:module.bzl", "go_module")

go_module(
    test_data = {
        "pkg/verify": ["pkg/testdata/**"],
    },
)
```

The test of `pkg/verify` then reads `../testdata/bundle.json`. A read of a file
that is not declared fails, so a test cannot depend on a file that yak does not
track. `go_package()` takes `test_data` as well.

A test that passed reports that pass in place of running again while the test
binary and the files it reads are unchanged, as with `go test`, and the output
shows `✓ Pass (cached)`. `yak test --no-test-cache` runs every test, as
`go test -count=1` does.
[Caching test results](../../../rule_authors/test_execution.md#caching-test-results)
lists what a pass depends on.
With `[build] remote_cache` set, passes are shared through a
[remote cache](../../remote_execution.md#remote-cache-without-remote-execution),
so a CI job runs only the tests whose inputs changed since a run that uploaded.

## Build files below the module's root

A build file in a directory below the module's root makes that directory a
package of its own, so `go_module()` cannot reach its files. That build file
calls `go_package()`, which declares the targets of the Go packages in its
directory and in the directories below it that have no build file of their own:

```python
# File: tools/YAK

load("@gomod//:module.bzl", "go_package")

go_package()

export_file(name = "notes.txt")
```

The package `tools/version` then has the target `//tools:version`. If a build
file that holds Go packages does not call `go_package()`, `go_module()` fails
and names its directory.

## Limitations

- The cell runs `go list` with `GOWORK=off`, so a `go.work` file does not
  apply. A project with several modules configures a cell for each.
- A module that vendors its dependencies in `vendor/` fails, and so does a
  `replace` directive that names a local directory.
- A package with internal tests (`package <name>` in a `_test.go` file) whose
  external tests import a package that imports it fails to build, because the
  test would link two builds of the package. `go test` builds such a dependency
  again against the package with its internal tests. A package with only
  external tests builds.
- A binary does not carry the module's version in its build information
  (`debug.ReadBuildInfo`), which `go build` takes from Git.
- The cell lists each platform with `CGO_ENABLED=1`. A target built with cgo
  disabled can lack a dependency that only its non-cgo files import.
