---
id: buck_out
title: yak-out
---

# yak-out

Buck2 stores build artifacts in a directory named `yak-out` in the root of your
[project](glossary.md#project). You should not make assumptions about where
Buck2 places your build artifacts within the directory structure beneath
`yak-out` as these locations depend on Buck2's implementation and could
potentially change over time. Instead, to obtain the location of the build
artifact for a particular target, you can use one of the `--show-*-output`
options with the [`yak build`](../../users/commands/build) or
[`yak targets`](../../users/commands/targets) commands, most commonly
`--show-output`. For the full list of ways to show the output location, you can
run `yak build --help` or `yak targets --help`.

```sh
yak targets --show-output <target>
yak build --show-output <target>
```
