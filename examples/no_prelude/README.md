## No-prelude example

This example project defines all of its rules and toolchains itself. Its
`.buckconfig` has no `prelude` cell:

```
#.buckconfig
[cells]
root = .
toolchains = toolchains
```

Each language directory holds its rules in `rules.bzl` (for example
`cpp/rules.bzl`). The `toolchains` cell holds the toolchains (for example
`toolchains/cpp_toolchain.bzl`).

## Sample commands

Install Buck2, cd into a project, and run

```bash
# List all targets
buck2 targets //...
# Build all targets
buck2 build //...
# Run C++ hello_world main
buck2 run //cpp/hello_world:main
```
