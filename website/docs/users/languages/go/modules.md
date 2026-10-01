---
id: modules
title: Go modules
---

# Go modules

A [`go` external cell](../../advanced/external_cells.md#the-go-origin) declares
the targets of a Go module from its `go.mod` and its sources. The build file in
the module's directory loads `go_module` from the cell and calls it:

```python
# File: YAK, next to go.mod

load("@gomod//:module.bzl", "go_module")

go_module()
```

The cell runs `go list` for macOS, Linux, and Windows on `amd64` and `arm64`,
and each target's dependencies select on the prelude's `os` and `cpu`
constraints where those platforms differ. Third-party packages build from the
versions that `go.mod` selects, checked against `go.sum`. A new import, a new
file, or a change to `go.mod` reaches the next build without a generator step.
The cell runs `go list` again only when the names of the module's files, the
build constraints, package clauses, imports, or `//go:embed` lines of its Go
files, or `go.mod` or `go.sum` change.

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
- The cell lists each platform with `CGO_ENABLED=1`. A target built with cgo
  disabled can lack a dependency that only its non-cgo files import.
