# Key actions on the identity of the tools they run

Follows [the plan contract](../../PLANS.md).

## Purpose

The key of an action covers its command line, its environment, and the contents of its inputs. The toolchains of `system_toolchains()` run `rustc`, `go`, and `clang` from `PATH`, and the tool binaries are not inputs. Two machines with different compiler versions compute the same key for an action, so a shared remote cache can return one compiler's output to a machine with another. A compiler upgrade on one machine leaves every local result in place as well, until an input of the action changes.

After this change, the key of each action that runs a system tool covers that tool's version output. An upgrade of `rustc` reruns the Rust actions and no others, and a remote cache keeps the results of different compilers apart.

The repository owner chose automatic identities per tool over a manual cache key and over hermetic toolchains (2026-10-01).

To see it working, build a Rust target, switch `rust-toolchain.toml` between two installed toolchains, and build again. The second build compiles the crate again. With a remote cache, a third build with the first toolchain reuses the first result.

## Progress

- [x] Milestone 1: the daemon computes `tool_identity.rustc`, `tool_identity.go`, and `tool_identity.clang` at the start of each command.
- [x] Milestone 2: the Rust, Go, and C++ system toolchains put the identities into the keys of their actions, and the `cargo` and `go` cells compute again when the identity of their tool changes.
- [x] Milestone 3: documentation and validation against a real toolchain switch.

## Surprises & Discoveries

- `ServerCommandContext::load_new_configs` (`app/yak_server/src/ctx.rs`) parses the configuration of each command with the client's `-c` overrides. Overrides reach DICE through `LegacyExternalYakConfigDataKey`, and `read_root_config` reads through a projection per section and key with equality, so an unchanged value invalidates nothing.
- `host_info::get_host_info` already computes external state on each command, such as the Xcode version on macOS, and injects it with equality.
- `cmd_script` (`prelude/utils/cmd_script.bzl`) returns `cmd_args(wrapper, hidden = cmd)`, so a hidden input of the `go` command reaches every Go tool wrapper.
- The Go toolchain copies `GOROOT` with a `go_copy_goroot` action whose key does not depend on the installed Go.
- The Rust prelude runs `rustc` through `RustToolchainInfo.compiler` in every action that compiles, including `prelude//rust/tools:rustc_cfg`, build scripts (`RUSTC`), and rustdoc's `--test-builder`.
- The `cargo` cell runs `cargo metadata` and `rustc --print cfg` in a DICE computation whose only inputs were the workspace's manifests and lock file, and the `go` cell runs `go list` in one whose inputs were the module's files. A toolchain switch kept the cells of the old toolchain.
- `[yak_re_client] tls` defaults to true, and the remote execution page did not document it until this change. Against a plaintext bazel-remote, every lookup and upload failed with `The service is currently unavailable`, and the build ran every action locally without reporting it.
- A first version passed the identities as `-c` overrides of the client. The external configuration log (`tests/core/build/test_external_yakconfigs.py`) then reported them with origin `cli`, as if the user had passed them, and its golden file would have held each machine's identities.
- `yak log what-uploaded` counts the input uploads of remote execution (`ReUpload` events). It reports 0 for a build whose local actions uploaded their results to a remote cache.

## Decision Log

- 2026-10-01: The daemon runs a fixed set of commands, which matches the tools of `system_toolchains()`: `rustc -vV`, `go version`, and `clang --version`. Each runs in the project root, so `rustup` selects the toolchain of `rust-toolchain.toml`, with the daemon's environment, which local actions inherit. The value is a digest of the exit status, stdout, and stderr, so a missing tool has a stable identity of its own.
- 2026-10-01: The identities enter the configuration as computed values (`ComputedConfigValue` in `app/yak_common/src/legacy_configs/args.rs`), which apply before the client's `-c` flags. A `-c tool_identity.rustc=...` from the user wins, which tests use to simulate a compiler change. The configuration components that a command's events report leave the computed values out.
- 2026-10-01: A toolchain carries the identity as a hidden input of its tools' `RunInfo`: a file that `ctx.actions.write` creates with the identity as its content. The command lines stay the same, and every action that runs the tool gets the file as an input.
- 2026-10-01: The `cargo` cell reads `tool_identity.rustc` and the `go` cell reads `tool_identity.go` through DICE, so their computations depend on the identity.
- 2026-10-01: Validation used a scratch copy of the test result cache's Cargo workspace in place of Roost. It switches between two installed toolchains in less than a minute, and a pinned identity reproduces the behavior before this change.

## Outcomes & Retrospective

Each action that runs `rustc`, `rustdoc`, `clippy-driver`, `go`, a tool of `GOROOT`, or the C compiler, assembler, or linker of `system_toolchains()` has the identity of its tool among its inputs.

Validation ran on 2026-10-01 against a scratch copy of `tests/core/test/test_test_result_cache_data/workspace` after `yak generate`, with the installed rustc 1.98.1 (stable) and rustc 1.98.0-nightly (2026-07-02). Checkouts A to D were copies of it with bazel-remote as the remote cache (`remote_cache = read_write`, `default_allow_cache_upload = true`, `tls = false`).

| Build | Actions |
| --- | --- |
| One checkout, stable, then nightly through `rust-toolchain.toml` | 2 ran (`rustc diag` and `failure_filter diag`) |
| The same checkout, nightly again | 0 ran |
| Checkout A, stable | 3 ran and uploaded |
| Checkout C, stable | 3 cache hits |
| Checkout B, nightly | 2 ran, and 1 cache hit for `deps`, which does not run `rustc` |
| Checkout B, stable | 2 cache hits |
| Checkout D, nightly with `-c tool_identity.rustc=<stable identity>` | 3 cache hits, the outputs of stable |

The last row is the behavior before this change, in which the key of an action did not depend on the compiler.

The identities cost each command about 21 ms (the tracker entry "Every command waits for the tool identities"). On Windows, the C++ toolchain carries the identity of clang while it runs MSVC (the tracker entry "The system C++ toolchain on Windows carries the identity of clang"). The identity is that of the tool on the machine that runs `yak`, and the remote execution page states that it does not describe the tools of remote workers.

If `rust-toolchain.toml` names a toolchain that `rustup` has not installed, `rustc -vV` installs it, and the 10-second limit can stop the install. That command then gets the identity of a timed-out run, and the next command computes the identity of the installed toolchain.

## Context and Orientation

- `app/yak_server/src/ctx.rs`: `ServerCommandContext::load_new_configs`.
- `app/yak_server/src/host_info.rs`: the precedent for external state computed on each command.
- `prelude/toolchains/system.bzl`: `system_toolchains()`.
- `prelude/toolchains/rust.bzl`, `prelude/toolchains/go/system_go_toolchain.bzl`, `prelude/toolchains/cxx.bzl`: the system toolchains.

## Plan of Work

1. A module of `yak_server` runs the identity commands with a timeout, in parallel, and returns computed configuration values. `load_new_configs` parses the configuration with them under the client's overrides. A unit test covers the digest of a missing tool. An integration test reads `tool_identity.rustc` with `yak audit config` and overrides it with `-c`.
2. `system_rust_toolchain`, `system_go_toolchain`, `system_go_bootstrap_toolchain`, and `system_cxx_toolchain` take `tool_identity`, and `system_toolchains()` passes `read_root_config("tool_identity", <tool>, "")`. An integration test builds a Rust target and a Go target, changes the identity with `-c`, and checks that the compile actions run again, and that a change of the Go identity leaves the Rust actions alone.
3. Documentation: the toolchain and remote cache pages, and `CHANGELOG.md`. The tech-debt entry for a shared cache closes. Validation switches Roost between the installed stable and nightly toolchains.

## Validation and Acceptance

- `python3 test.py` passes, because the change reaches `yak_common`.
- `tests/core/prelude/test_tool_identity.py` passes.
- In a Cargo workspace with a remote cache, a build after a switch of `rust-toolchain.toml` compiles the workspace again, and a build after a switch back gets its results from the cache.

## Idempotence and Recovery

An identity that changes reruns the affected actions once. Setting `-c tool_identity.<tool>=<value>` pins an identity, for example while investigating a rebuild.
