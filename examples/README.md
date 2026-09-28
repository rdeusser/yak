# buck2 examples

In these folders are some examples on how to get buck2 working with your
favorite languages and tools.

## with_prelude

Examples taking advantage of the prelude to create toolchain-independent build
definitions in cpp and python. Includes as an example a usecase for building and
using c-extension-backed python libraries.

The project uses the prelude bundled with the `buck2` binary, which its
`.buckconfig` selects with `[external_cells] prelude = bundled`.

## no_prelude

Preludeless examples for those wanting to use buck2 with their own rules and
toolchains. In here you can learn about how BUILD files interact with rules, and
how the provider abstraction can be used to encapsulate build logic.

## toolchains

Examples testing the various toolchains included in the prelude.

## bootstrap

A sample project that demonstrates configuration of a bootstrap toolchain.
