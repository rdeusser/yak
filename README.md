<div class="title-block" style="text-align: center;" align="center">

# yak: fast multi-language build system

![Version] ![License] [![Build Status]][CI]

[Version]:
  https://img.shields.io/badge/release-unstable,%20"Developer%20Edition"-orange.svg
[License]:
  https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blueviolet.svg
[Build Status]:
  https://github.com/rdeusser/yak/actions/workflows/build-and-test.yml/badge.svg
[CI]: https://github.com/rdeusser/yak/actions/workflows/build-and-test.yml

<strong>
  <a href="https://rdeusser.github.io/yak/">Homepage</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="https://rdeusser.github.io/yak/docs/getting_started/">Getting Started</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="./CONTRIBUTING.md">Contributing</a>
</strong>

---

</div>

yak is a fast, hermetic, multi-language build system. It is an independent
open-source fork of Yak, which Meta created.

But what do those words really mean for a build system &mdash; and why might
they interest you? "But why yak?" you might ask, when so many build systems
already exist?

- **Fast**. It doesn't matter whether a single build command takes 60 seconds to
  complete, or 0.1 seconds: when you have to build things, yak doesn't waste
  time &mdash; it calculates the critical path and gets out of the way, with
  minimal overhead. It's not just the core design, but also careful attention to
  detail that makes yak so snappy. So you spend more time iterating, and less
  time waiting.
- **Hermetic**. When using Remote Execution[^hermetic-re-only], yak becomes
  _hermetic_: it is required for a build rule to correctly declare all of its
  inputs; if they aren't specified correctly (e.g. a `.c` file needs a `.h` file
  that isn't correctly specified), the build will fail. This enforced
  correctness helps avoids entire classes of errors that most build systems
  allow, and helps ensure builds work everywhere for all users. That means "it
  compiles on my machine" can become a thing of the past.
- **Multi-language**. Many teams have to deal with multiple programming
  languages that have complex inter-dependencies, and struggle to express that.
  Most people settle with `make` and tie together `dune` to `pip` and `cargo`.
  But then how do you run test suites, code coverage, or query code databases?
  yak is designed to support multiple languages from the start, with
  abstractions for interoperation. And because it's completely scriptable, and
  _users_ can implement language support &mdash; it's incredibly flexible. Now
  your Python library can depend on an OCaml library, and your OCaml library can
  depend on a Rust crate &mdash; and with a single build tool, you have a
  consistent UX to build and test and integrate all of these components.

[^hermetic-re-only]:
    yak currently does not sandbox _local-only_ build steps; in contrast,
    yak using Remote Execution is _always_ hermetic by design. The vast
    majority of build rules are remote compatible, as well. Despite that, we
    hope to lift this restriction in the (hopefully short-term) future so that
    local-only builds are hermetic as well.

If you're familiar with systems like [Bazel](https://bazel.build/) or
[Pants](https://www.pantsbuild.org/) &mdash; then yak will feel warm and cozy,
and these ideas will be familiar. But then why create yak if those already
exist? Because that isn't all &mdash; the page
_["Why yak?"](https://rdeusser.github.io/yak/docs/about/why/)_ on our website goes into
more detail on several other important design criteria that separate yak from
the rest of the pack, including:

- Support for ultra-large repositories, through watching for changes to the
  filesystem.
- Totally language-agnostic core executable, with a small API &mdash; even C/C++
  support is written as a library. You can write everything from scratch, if you
  wanted.
- BXL, the Starlark extension language of yak, can be used for
  self-introspection of the build system, allowing automation tools to inspect
  and run actions in the build graph. This allows you to more cleanly support
  features that need graph introspection, like LSPs or compilation databases.
- Support for distributed compilation, using the same Remote Execution API that
  is supported by Bazel. Existing solutions like BuildBarn, BuildBuddy, EngFlow,
  and NativeLink all work today.
- An efficient, robust, and sound design &mdash; inspired by modern theory of
  build systems and incremental computation.
- And more!

If these headline features make you interested &mdash; check out the
[Getting Started](https://rdeusser.github.io/yak/docs/getting_started/) guide!

## 🚧🚧🚧 **Warning** 🚧🚧🚧 &mdash; rough terrain lies ahead

yak currently **does not have a stable release tag at this time**. Pre-release
tags/binaries, and stable tags/binaries, will come at later dates. Tracking the
latest commit on `main` is the best way to report bugs and catch regressions.

Expect rough edges. Several features are missing or in progress, and you'll
probably have to fiddle with things more than necessary to get it nice and
polished.

Please provide feedback by submitting
[issues and questions!](https://github.com/rdeusser/yak/issues)

## Installing yak

You can get started by downloading a
[tagged version](https://github.com/rdeusser/yak/tags) or the
[latest](https://github.com/rdeusser/yak/releases/tag/latest) built binary for
your platform. The `latest` tag always refers to a recent commit; it is updated
on every single push to the GitHub repository, so it will always be a recent
version.

Alternately, you can use [dotslash](https://dotslash-cli.com/) with the tagged
releases where it's easy to deploy into a repo with a single text file and auto
pull the correct platform as needed.

You can also compile yak from source, if a binary isn't immediately available for your use; check
out the [docs](https://rdeusser.github.io/yak/docs/getting_started/install/) for information.

## Terminology conventions

Frequently used terms and their definitions can be found on the
[glossary page](https://rdeusser.github.io/yak/docs/concepts/glossary/).

## License

yak is licensed under both the MIT license and Apache-2.0 license; the exact
terms can be found in the [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE) files, respectively.
