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
daemon computes an identity for each. It runs `rustc -vV`, `go version`, and
`clang --version` in the project root, where `rustup` reads
`rust-toolchain.toml`, and sets `tool_identity.rustc`, `tool_identity.go`, and
`tool_identity.clang` to digests of their output. Each toolchain of
`system_toolchains` puts its tool's identity into the key of every action that
runs the tool. After a compiler upgrade, the actions that run that compiler run
again, and the actions of other languages keep their results.
`-c tool_identity.rustc=<value>` sets an identity in place of the computed one.

The daemon keeps each identity and runs the version command again at the next
command when the metadata of a file that decides its output changed:

- the tool's file on the `PATH`, the files of the same name earlier on the
  `PATH`, and the file that its links lead to;
- for `rustc`, `rust-toolchain.toml` and `rust-toolchain` in the project root
  and its ancestors, rustup's `settings.toml`, and the `rustc` of the toolchain
  that `rustc --print sysroot` names;
- for `go`, `go.mod` and `go.work` in the project root and its ancestors, the
  file of `go env -w`, and `go.env` of the Go installation;
- for `clang` on macOS, `/var/db/xcode_select_link` and the compiler that
  `xcrun --find clang` names when the `PATH` leads to `/usr/bin/clang`.

The daemon reads its environment, such as `PATH`, `RUSTUP_HOME`, and
`RUSTUP_TOOLCHAIN`, once when it starts. A change that none of these files
records keeps the old identity until `yak kill` stops the daemon.

For more information about defining toolchains, see the
[relevant page in the Rule Authors section](../rule_authors/writing_toolchains.md).
