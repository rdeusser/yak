# @configuration syntax

## What

Command

```shell
yak build //foo:bar@config//platform:linux-x86_64
```

should be equivalent to current syntax:

```shell
yak build //foo:bar --target-platforms=//platform:linux-x86_64
```

## Why

Might be convenient if we define global (or per-target) alias. For example, if
there's an alias

```
release=//config:linux-x86_64-release
```

The command above can be expressed as:

```shell
yak build //foo:bar@release
```

Additionally, if we have configuration expressions implemented, we can do
something like:

```shell
yak build //foo:bar@release+gcc
```

## Possible future extensions

For now, at-syntax only applies to command line arguments

- of `build`/`targets`/`run`/`test` commands
- probably `cquery` query

It would be reasonable to expect that this syntax should be allowed anywhere we
need a target (e.g. in `deps` attribute), but this is out of scope of this
proposal.
