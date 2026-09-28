---
id: index
title: Introduction
---

Welcome to Buck2, a large scale, fast, reliable, and extensible build tool.
Buck2 supports a variety of languages on many platforms.

This project is an independent open-source fork of Buck2, which Meta created.

Buck2's core is written in [Rust](https://www.rust-lang.org/).
[Starlark](https://github.com/bazelbuild/starlark), which is a deterministic,
immutable dialect of Python, is used to extend the Buck2 build system, enabling
Buck2 to be language-agnostic. With Starlark, users can define their own custom
rules.

Buck2 leverages the Bazel spec of
[Remote Build Execution](https://bazel.build/remote/rbe) as the primary means of
parallelization and caching, which increases the importance of idempotency (no
matter how many times an operation is performed, it yields the same result) and
hermeticity (code is sealed off from the world), giving the right results,
reliably.

Buck2 multi-language support includes C++, Python, Java, Kotlin, Go, Rust,
Erlang, OCaml, and more.

The following sub-sections contain a list of links to key points in the Buck2
Documentation website that explain the advantages of using Buck2 for you and
your team.

## Buck2 Documentation Website Links

### For end users

- [Getting Started](getting_started/index.md) - how to get started with using
  Buck2.
- [Benefits](about/benefits/compared_to_buck1.md) - the benefits of using Buck2.

### For people writing rules

- [Writing Rules](rule_authors/writing_rules.md) - how to write rules to support
  new languages.
- [Build APIs](api/build) - documentation for the APIs available when writing
  rules.
- [Loading Data](users/loading_data.md) - How to load static data from JSON and
  TOML files in rules.
- [Starlark Types](https://github.com/rdeusser/buck2/blob/main/starlark-rust/docs/types.md) -
  rules are written in Starlark (which is approximately Python), and the Buck2
  implementation of Starlark adds types.

### For people integrating with Buck2

- [Extending Buck via BXL](./bxl) - powerful Starlark scripts for introspection
  of Buck2's graphs.
- [Buck2 change detector](https://github.com/facebookincubator/buck2-change-detector) -
  tools for building a CI that only builds/tests what has changed in diff/PR.
- [Buck2 GitHub actions installer](https://github.com/dtolnay/install-buck2) -
  script to make GitHub CI with Buck2 easier.
- [Reindeer](https://github.com/facebookincubator/reindeer) - a set of tools for
  importing Rust crates from crates.io, git repos etc and generating a BUCK file
  for using them.
- [ocaml-scripts](https://github.com/facebook/ocaml-scripts) - scripts to
  generate a BUCK file enabling the use of OCaml packages from an OPAM switch.
- [Buckle](https://github.com/benbrittain/buckle) - a launcher for Buck2 on a
  per-project basis. Enables a project or team to do seamless upgrades of their
  build system tooling.

### External articles about Buck2

- [Introducing Buck2](https://engineering.fb.com/2023/04/06/open-source/buck2-open-source-large-scale-build-system/) -
  the announcement of the open-source release of Buck2.
- [Reddit AMA](https://old.reddit.com/r/rust/comments/136qs44/hello_rrust_we_are_meta_engineers_who_created_the/)
  where the original Buck2 developers answered a number of questions.
- [Using buck to build Rust projects](https://steveklabnik.com/writing/using-buck-to-build-rust-projects) -
  working through an initial small Rust project, by
  [Steve Klabnik](https://steveklabnik.com/). Followed up by
  [building from crates.io](https://steveklabnik.com/writing/using-cratesio-with-buck)
  and [updating Buck2](https://steveklabnik.com/writing/updating-buck).
- [Awesome Buck2](https://github.com/sluongng/awesome-buck2) is a collection of
  resources about Buck2.
- [Buck2 Unboxing](https://www.buildbuddy.io/blog/buck2-review/) is a general
  review of Buck2 by [Son Luong Ngoc](https://github.com/sluongng/).
- [A tour around Buck2](https://www.tweag.io/blog/2023-07-06-buck2/) gives an
  overview of Buck2 and how it differs from Bazel.

### External videos about Buck2

- [Accelerating builds with Buck2](https://www.youtube.com/watch?v=oMIzKVxUNAE)
  explains why Buck2 is fast.
- [Buck2: optimizations & dynamic dependencies](https://www.youtube.com/watch?v=EQfVu42KwDs)
  explains why Buck2 is fast and covers some of the advanced dependency
  features.
- [Building Erlang with Buck2](https://www.youtube.com/watch?v=4ALgsBqNBhQ)
  covers building a large Erlang code base with Buck2.
- [antlir2: Deterministic image builds with Buck2](https://www.youtube.com/watch?v=Wv-ilbckSx4)
  talks about layering a packaging system over Buck2.

### External projects using Buck2

- [System Initiative](https://www.systeminit.com/) build their DevOps product
  [using Buck2](https://nickgerace.dev/post/system-initiative-the-second-wave-of-devops/#under-the-hood),
  with their own custom prelude.
- [Rust `cxx` library](https://github.com/dtolnay/cxx) has examples and tests
  with a wide variety of build systems, including Buck2.
- [`ocamlrep` library](https://github.com/facebook/ocamlrep) allows for interop
  between OCaml and Rust code, and can be
  [built with Buck2](https://github.com/facebook/ocamlrep/blob/main/README-BUCK.md).
- [`buck2-nix`](https://github.com/thoughtpolice/buck2-nix) is an experiment to
  integrate Buck2, [Sapling](https://sapling-scm.com) and
  [Nix](https://nixos.org) together in a harmonious way.

Feel free to
[send a PR](https://github.com/rdeusser/buck2/edit/main/website/docs/index.md)
adding your project.

### For people developing Buck2

- [`docs/developers/basics.md`](https://github.com/rdeusser/buck2/blob/main/docs/developers/basics.md)
  covers building, testing, and the coding conventions of the source tree.
