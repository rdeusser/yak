---
id: install
title: Installing Buck2
---

## Installing Buck2

The latest set of `yak` executables can be found under the
[`latest` release page](https://github.com/rdeusser/buck2/releases/tag/latest).

Additionally, for each bi-monthly release there is a
[dotslash](https://dotslash-cli.com) file that is appropriate for committing to
a repository. This will automatically fetch the correct version and architecture
for each user, and ensures a consistent build environment for each commit in the
repo.

If no prebuilt binary is available for your platform — or you want to hack on
Buck2 itself — see [Building from Source](#building-from-source) below.

## Building from Source

Buck2 currently requires a nightly Rust toolchain. The simplest setup is via
[rustup](https://rustup.rs/), which provisions the right `rustc`/`cargo` for
you. Once it's installed, build and install `yak` directly from GitHub:

```bash
rustup install nightly-2026-07-05
cargo +nightly-2026-07-05 install --git https://github.com/rdeusser/buck2.git buck2
```

This installs `yak` into a suitable directory such as `$HOME/.cargo/bin`,
which you should add to your `$PATH`:

Linux / macOS

```sh
export PATH=$HOME/.cargo/bin:$PATH
```

Windows Powershell

```powershell
$Env:PATH += ";$HOME\.cargo\bin"
```

Verify the install with `yak --help`.

To hack on Buck2, build from a clone of the repo instead:

```sh
git clone https://github.com/rdeusser/buck2.git
cd buck2/
cargo install --path=app/buck2
```

### Using Nix

Most [Nix](https://nixos.org/nix) users provision tools directly with Nix
itself, rather than rustup. The Buck2 source ships a `flake.nix` that exposes a
`cargo`/`rustc` development shell:

```sh
git clone https://github.com/rdeusser/buck2.git
cd buck2/
nix develop . # add 'rustc' and 'cargo' to $PATH
cargo build --release --bin=yak
```

A Nix package (e.g. `nix build .#buck2`) does not yet exist; see `yak` in
nixpkgs for inspiration for writing one. An `.envrc` using the Nix flake is
provided for `direnv` users — `direnv allow` will give a usable development
environment.

### `protoc` on non-Tier-1 platforms

Buck2 uses Protocol Buffers extensively, both internally and to talk to remote
systems for things like Remote Execution. Compiling the `.proto` files needs
the `protoc` compiler.

On Linux (aarch64/x86_64), Windows, and macOS the `cargo` build pulls a
prebuilt `protoc` from the
[`protoc-bin-vendored`](https://crates.io/crates/protoc-bin-vendored) crate, so
no setup is required.

On other operating systems, install `protoc` from another source (out of scope
here) and point the build at it before running `cargo build`:

- `YAK_BUILD_PROTOC` — path to the `protoc` binary
- `YAK_BUILD_PROTOC_INCLUDE` — path to the protocol buffers header directory

For example, with protobuf installed under `/opt/protobuf`:

```bash
export YAK_BUILD_PROTOC=/opt/protobuf/bin/protoc
export YAK_BUILD_PROTOC_INCLUDE=/opt/protobuf/include
```

### Building Buck2 with Buck2

See [Bootstrapping](../about/bootstrapping.md) for details. The gist:

```sh
cargo build --bin=yak
reindeer --third-party-dir third-party/rust buckify
target/debug/yak build //:yak
```
