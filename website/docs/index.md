---
id: index
title: Introduction
---

Welcome to yak, a large scale, fast, reliable, and extensible build tool.
yak supports a variety of languages on many platforms.

This project is an independent open-source fork of Yak, which Meta created.

yak's core is written in [Rust](https://www.rust-lang.org/).
[Starlark](https://github.com/bazelbuild/starlark), which is a deterministic,
immutable dialect of Python, is used to extend the yak build system, enabling
yak to be language-agnostic. With Starlark, users can define their own custom
rules.

yak leverages the Bazel spec of
[Remote Build Execution](https://bazel.build/remote/rbe) as the primary means of
parallelization and caching, which increases the importance of idempotency (no
matter how many times an operation is performed, it yields the same result) and
hermeticity (code is sealed off from the world), giving the right results,
reliably.

yak multi-language support includes C++, Python, Go, Rust, Erlang, OCaml, and
more.

The following sub-sections contain a list of links to key points in the yak
Documentation website that explain the advantages of using yak for you and
your team.

## Where to start

### For end users

- [Getting Started](getting_started/index.md) - how to get started with using
  yak.
- [Why yak](about/why.md) - what yak does differently from other build systems.

### For people writing rules

- [Writing Rules](rule_authors/writing_rules.md) - how to write rules to support
  new languages.
- [Build APIs](api/build) - documentation for the APIs available when writing
  rules.
- [Loading Data](users/loading_data.md) - How to load static data from JSON and
  TOML files in rules.
- [Starlark Types](https://github.com/rdeusser/yak/blob/main/starlark-rust/docs/types.md) -
  rules are written in Starlark (which is approximately Python), and the yak
  implementation of Starlark adds types.

### For people integrating with yak

- [Extending yak via BXL](./bxl) - powerful Starlark scripts for introspection
  of yak's graphs.

### For people developing yak

- [`docs/developers/basics.md`](https://github.com/rdeusser/yak/blob/main/docs/developers/basics.md)
  covers building, testing, and the coding conventions of the source tree.
