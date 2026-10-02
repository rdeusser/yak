---
id: toolchain
title: Toolchain
---

A toolchain defines a set of tools, scripts and flags used by certain rules.
Their purpose is to enable reuse of rules across projects that source their
tools (e.g. compilers and linters) differently.

For example, consider `cxx_binary`, which is defined in the prelude. Since
building C++ code is complex, it is desirable to share the same implementation
of `cxx_binary` across projects. However, not all projects will want to source
their C++ compiler, linker, etc. the same way:

- Some projects do not care, and want to pick them from the ambient environment
  (most likely the tools installed system-wide).
- Some projects want to achieve reproducible builds by running the build within
  some sort of virtual environment.
- Some projects want to achieve reproducible builds by downloading tools as part
  of the build itself.
- Some projects want to achieve reproducible builds by accessing tools checked
  into version control.

Defining those in a toolchain lets us decouple those project-specific concerns
from generic build rules.

`yak init` writes a `toolchains` cell that calls the `system_toolchains`
macro, which declares a toolchain for each language that runs the compilers and
tools on the `PATH`.

The compilers on the `PATH` are not inputs of the actions that run them, so the
daemon computes an identity for each at the start of every command. It runs
`rustc -vV`, `go version`, and `clang --version` in the project root, where
`rustup` reads `rust-toolchain.toml`, and sets `tool_identity.rustc`,
`tool_identity.go`, and `tool_identity.clang` to digests of their output. Each
toolchain of `system_toolchains` puts its tool's identity into the key of every
action that runs the tool. After a compiler upgrade, the actions that run that
compiler run again, and the actions of other languages keep their results.
`-c tool_identity.rustc=<value>` sets an identity in place of the computed one.

For more information about defining toolchains, see the
[relevant page in the Rule Authors section](../rule_authors/writing_toolchains.md).
