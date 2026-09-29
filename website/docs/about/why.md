---
id: why
title: Why yak
---

yak is a build system for large, multi-language repositories. This page
answers the questions:
[why does yak exist](#why-does-yak-exist),
[what's different about yak](#whats-different-about-yak), and
[why use yak](#why-use-yak).

## Why does yak exist?

Some organizations keep their code in a very large monorepo, consisting of a
variety of programming languages, including C++, Python, Rust, Go, Swift,
Objective-C, Haskell, OCaml, and more.

These large scale and multi-language repositories are generally beyond the
capabilities of traditional build systems like `make`. yak is a fork of Yak,
which Meta wrote for such a repository. Its design borrows ideas from
[academic](https://ndmitchell.com/#shake_10_sep_2012)
[research](https://ndmitchell.com/#shake_21_apr_2020) and build systems,
including [Bazel](https://bazel.build/), [Pants](https://www.pantsbuild.org/),
[Shake](https://shakebuild.com/), [Tup](https://gittup.org/tup/), and more.

yak has these features, and in most cases Bazel has them too:

- **Targets that can be queried** - the build is defined as a series of targets,
  specified in `YAK` files, that depend on other targets. This graph of targets
  can be queried to understand how they relate to each other and what the
  potential impact of a change might be.
- **Remote execution** - the build can send actions to a set of remote servers
  to be executed, increasing the parallelism significantly.
- **Multi-language composability** - there can be lots of different languages in
  a single build, and they can be put together. For example, you could have a
  Python library that depends on a Rust library, which, in turn depends on a C
  library.
- **File watching** - at large enough scale, simply looking for changed files is
  prohibitively expensive. yak can integrate with
  [Watchman](https://facebook.github.io/watchman/) to discover which files have
  changed efficiently. For simplicity of setup, yak defaults to using
  `inotify` or similar functionality.
- **Uses Starlark** - Starlark is a deterministic Python-like language used to
  specify the targets, enabling the definition of targets as literals and more
  advanced manipulation/sharing.

## What's different about yak?

Several features of yak give it efficiency or expressiveness that most build
systems, including Bazel, do not have:

- **yak is written in Rust** - it has no garbage collection pauses.
- **yak is remote execution first** - local execution is considered a special
  case of remote execution. That means that things such as directory hashes can
  be pre-computed ready to send to remote execution, giving efficiency benefits.
- **All yak rules are written in Starlark** - the rules live outside the binary,
  which makes iteration on rules much faster.
- **The yak binary is entirely language agnostic** - as a consequence of
  having all the rules external to the binary, the most important and complex
  rule (such as in C++), don't have access to magic internal features. As a
  result, features have been made available to all rules, including:
  - [Dep files](../rule_authors/dep_files.md) - the ability to declare that a
    subset of the files weren't actually used, and thus not be sensitive to
    changes within them.
  - [Incremental actions](../rule_authors/incremental_actions.md) - the ability
    to have the action short-circuit some subset of the work if run again.
- **yak uses a dynamic (aka monadic) graph as its underlying computation
  engine** - while most dependencies are specified statically, there are two
  particular features that expose dynamic power to rule authors:
  - [Dynamic dependencies](../rule_authors/dynamic_dependencies.md) - enable
    rules to build a file then look at its contents before specifying the
    dependencies and steps in future actions. Common uses are languages where
    the dependency structure within a project must follow imports (e.g. Haskell,
    OCaml) and distributed ThinLTO (where the best optimization plan is
    generated from summaries).
  - [Anonymous targets](../rule_authors/anon_targets.md) - enable rules to
    create a graph that has more sharing than the original user graph. As a
    result, two unrelated binaries can compile shared code only once, despite
    the shared code not knowing about this commonality. This feature is useful
    for rules like Swift feature resolution.
- **[Transitive-sets](../rule_authors/transitive_sets.md)** - similar in purpose
  to Bazel's [depset](https://bazel.build/rules/lib/depset). But, instead of
  being just a memory optimization, are also wired into the dependency graph,
  providing a reduction in the size of the dependency graph.
- **yak is not phased** - there are no target graph/action graph phases, just
  a series of dependencies in a
  [single graph on DICE](https://github.com/rdeusser/yak/blob/main/dice/dice/docs/index.md)
  that result in whatever the user requested. That means that yak can
  sometimes parallelise different phases and track changes very precisely.
- **The yak Starlark implementation is available
  [as a standalone library](https://github.com/rdeusser/yak/tree/main/starlark-rust)** -
  this provides features such as IDE integration (both LSP and DAP bindings),
  linters, typecheckers, and more. These features are integrated into yak to
  give a better developer experience (which is still evolving).
- **yak supports configurations** - (such as `select`) to provide
  multi-platform/architecture builds, which are heavily inspired by Bazel.
  Within that space, there is a number of small differences, such as
  `toolchain_deps`.

## Why use yak?

It would be delightful if you tried out yak! But it is early-stage software,
so users may run into unexpected issues. If you encounter an issue, please
report it via [GitHub issues](https://github.com/rdeusser/yak/issues).

There are some things that aren't quite yet finished:

- There are not yet mechanisms to build in release mode (that should be achieved
  by modifying the toolchain).

If none of that puts you off, [give yak a go](../getting_started/index.md)!
