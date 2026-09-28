---
id: bootstrapping
title: Bootstrapping Buck2
---

# Bootstrapping Buck2

Buck2 can be built with `cargo` or `buck2`. The Buck build of the source
repository needs a `buck2` binary built from the same source, so build one with
`cargo` first:

```sh
cargo build --bin=buck2
```

For dependencies on Rust crates from [crates.io](https://crates.io), we use
[reindeer](https://github.com/facebookincubator/reindeer) to automatically
generate `BUCK` files. The source repository includes a
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

Build a copy of `buck2` with `buck2`:

```sh
target/debug/buck2 build //:buck2
```
