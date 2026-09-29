---
id: bootstrapping
title: Bootstrapping yak
---

# Bootstrapping yak

yak can be built with `cargo` or `yak`. The yak build of the source
repository needs a `yak` binary built from the same source, so build one with
`cargo` first:

```sh
cargo build --bin=yak
```

For dependencies on Rust crates from [crates.io](https://crates.io), we use
[reindeer](https://github.com/facebookincubator/reindeer) to automatically
generate `YAK` files. The source repository includes a
[DotSlash](https://dotslash-cli.com) file that runs `reindeer`.

Note that the resulting binary will be compiled without optimisations or
[jemalloc](https://github.com/jemalloc/jemalloc), so we recommend using the
Cargo-produced binary in further development.

First, install `dotslash` with `Cargo`:

```sh
cargo install --locked dotslash
```

Next, use `reindeer` to buckify dependencies:

```sh
cd buck2/
./bootstrap/reindeer --third-party-dir third-party/rust buckify
```

Build a copy of `yak` with `yak`:

```sh
target/debug/yak build //:yak
```
