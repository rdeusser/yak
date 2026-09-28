## No-prelude example

This example project defines all of its rules and toolchains itself. Its
`.yakconfig` has no `prelude` cell:

```
#.yakconfig
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
yak targets //...
# Build all targets
yak build //...
# Run C++ hello_world main
yak run //cpp/hello_world:main
```
