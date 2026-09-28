# Website

This directory holds the documentation site, which [Docusaurus](https://docusaurus.io/) builds from the pages in `docs/`.

## Installation

```shell
yarn install
```

## Generated content

`gen_docs.py` runs `buck2` to write the Starlark API reference, the prelude rule pages, the command reference, and the query function pages into `docs/`. Git ignores its output. Rerun it to see changes to generated content.

```shell
# Use the buck2 on PATH
yarn generate
# Build buck2 from source with ./buck2.py, which needs the Buck build of this repository
# and a buck2 on PATH built from this repository
yarn generate_local
# Build buck2 from source with Cargo
./gen_docs.py --cargo
```

## Local development

```shell
yarn start
```

This command starts a development server at `http://localhost:3000/buck2/`. It reloads a page when its Markdown in `docs/` changes. Restart the server after changing the site configuration.

## Production build

`yarn build` generates the reference pages and writes the static site to `build/`. `yarn serve` serves `build/` locally. `yarn build_cargo` and `yarn build_prebuilt` do the same with a Cargo build of `buck2` or with the binary that `BUCK2_BIN` names.

## Deployment

`.github/workflows/upload_buck2.yml` builds the site and publishes `build/` to the `gh-pages` branch on every push to `main`. GitHub Pages serves that branch at `https://rdeusser.github.io/buck2/` once the repository's Pages settings select it.

To deploy from a local checkout, generate the reference pages first, because `yarn deploy` builds the site without running `gen_docs.py`:

```shell
yarn generate
GIT_USER=<Your GitHub username> USE_SSH=true yarn deploy
```

## Fixing GitHub Security Alerts

This package carries both `package-lock.json` and `yarn.lock`, and a dependency
pin has to be applied to both. See
[`docs/developers/js_dependencies.md`](../docs/developers/js_dependencies.md).
