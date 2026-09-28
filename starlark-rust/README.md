# Starlark in Rust

This project provides a Rust implementation of the
[Starlark language](https://github.com/bazelbuild/starlark/blob/master/spec.md).
Starlark (formerly codenamed Skylark) is a deterministic language inspired by
Python3, used for configuration in build systems such as
[Bazel](https://bazel.build) and the build system in this repository, which
depends on this library. This project was originally developed
[in this repository](https://github.com/google/starlark-rust), which contains a
more extensive history.

There are at least three implementations of Starlark,
[one in Java](https://github.com/bazelbuild/starlark),
[one in Go](https://github.com/google/starlark-go), and this one in Rust. We
mostly follow the Starlark standard. To try out Rust Starlark, run this from
the repository root:

```shell
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ cargo run --package starlark_bin
$> 1+2
3
```

This project was started by
[Damien Martin-Guillerez](https://github.com/damienmg) at
[Google](https://github.com/google/starlark-rust), and continued in the Buck2
repository from version 0.4.0.

## Features

This project features:

- Easy interoperability between Rust types and Starlark.
- Rust-friendly types, so frozen values are `Send`/`Sync`, while non-frozen
  values aren't.
- [Garbage collected](docs/gc.md) values allocated on a heap.
- Optional runtime-checked [types](docs/types.md).
- A linter, to detect code issues in Starlark.
- IDE integration in the form of
  [LSP](https://microsoft.github.io/language-server-protocol/).
- Extensive testing, including
  [fuzz testing](https://github.com/google/oss-fuzz/tree/master/projects/starlark-rust).
- [DAP](https://microsoft.github.io/debug-adapter-protocol/) support.

This project also has two non-goals:

- We do _not_ aim for API stability between releases, preferring to iterate
  quickly and refine the API as much as possible. But we do
  [follow SemVer](https://doc.rust-lang.org/cargo/reference/semver.html).
- We do _not_ aim for minimal dependencies, preferring to keep one package with
  lots of power. But if some dependencies prove tricky, we might add feature
  flags.

## Components

There are six components:

- `starlark_derive`, a proc-macro crate that defines the necessary macros for
  Starlark. This library is a dependency of `starlark` the library, which
  reexports all the relevant pieces, and should not be used directly.
- `starlark_map`, a library with memory-efficient ordered/unordered maps/sets
  and various other data structures useful in Starlark.
- `starlark_syntax`, a library with the AST of Starlark and parsing functions.
  Only use if you want to manipulate the AST directly.
- `starlark` the main library, with evaluator, standard library, debugger
  support and lots of other pieces. Projects wishing to embed Starlark in their
  environment (with additional types, library functions and features) will make
  use of this library. This library reexports the relevant pieces of
  `starlark_derive`, `starlark_map` and most of `starlark_syntax`.
- `starlark_lsp`, a library providing an
  [LSP](https://microsoft.github.io/language-server-protocol/).
- `starlark_bin` the binary, which provides interactive evaluation, IDE features
  and linter, exposed through a command line. Useful if you want to use vanilla
  Starlark (but if you do, consider Python3 instead) or as a test-bed for
  experimenting. Most projects will end up implementing some of this
  functionality themselves over the `starlark` and `starlark_lsp` libraries,
  incorporating their specific extra types etc.

In particular the `starlark_bin` binary _can_ be effectively used as a linter.
But for the REPL, evaluator and IDE features the `starlark_bin` binary is only
aware of standard Starlark. Most Starlark embeddings supply extra functions and
data types to work with domain-specific concerns, and the lack of these bindings
will cause the REPL/evaluator to fail if they are used, and will give a subpar
IDE experience. In most cases you should write your own binary depending on the
`starlark` library, integrating your domain-specific pieces, and then using the
bundled LSP functions to produce your own IDE/REPL/evaluator on top of those.
You should still be able to use the [VS Code extension](vscode/README.md).

## Compatibility

In this section we outline where we don't comply with the
[Starlark spec](https://github.com/bazelbuild/starlark/blob/master/spec.md).

- We have plenty of extensions, e.g. type annotations, recursion, top-level
  `for`.
- We don't yet support later additions to Starlark, such as bytes.
- In some cases creating circular data structures may lead to stack overflows.

## License

Starlark Rust is Apache License, Version 2.0 licensed, as found in the
[LICENSE-APACHE](../LICENSE-APACHE) file.
