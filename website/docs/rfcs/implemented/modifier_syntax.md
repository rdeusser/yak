# [RFC] `?modifier` syntax

`?modifier` syntax for target patterns on CLI allows building with multiple combinations of modifiers. This enables building in multiple configurations at the same time from the same CLI invocation

## Basic syntax

The following is copied from the command-line modifiers section of the original [modifiers RFC](modifiers.md).

Modifiers from `?` syntax are specified as `buck2 build <target pattern>?<modifiers separated by plus signs>`.

For example, `buck2 build repo//foo:bar?prelude//constraints/sanitizer:asan` applies asan modifier on the command line. `buck2 build repo//foo:bar?prelude//constraints/os:linux+prelude//constraints/sanitizer:asan` will apply linux and asan modifiers.

Modifiers can be specified for any target pattern, so `buck2 build repo//foo/...?asan` and `buck2 build repo//foo:?asan` are both valid.

When specifying a subtarget and modifier with `?`, subtarget should go before the modifier, ex. `buck2 build repo//foo:bar[comp-db]?asan`. This configures `repo//foo:bar` against `asan` modifier and then builds just the `comp-db` subtarget.

It is prohibited to specify both `--modifier` flag and `?` on CLI. This restriction may be removed in the future after implementation of this RFC provided we see good motivation for it.

`?modifier` syntax is only allowed on CLI and certain parts of BXL that are CLI-like. It is only meant to express convenient configurations on CLI. It will be strictly prohibited on any non-CLI surfaces like BUCK files.

## `--show-output`

Buck’s build commands accept a set of `--show-output` flags (ex. `--show-output` and `--show-full-output`) that prints the output location of targets specified on CLI. For example, invoking `buck2 build repo//foo:bin –show-output` prints

```python
repo//foo:bin buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin
```

Likewise, invoking build in a different mode like `buck2 build repo//foo:bin -m opt` will print a different path

```python
repo//foo:bin buck-out/v2/gen/repo/b706492dec65e54c/foo/bin
```

With `?`-syntax, users would be able to invoke `buck2 build repo//foo:bin repo//foo:bin?opt` in the same invocation. For that, we propose to use the following output structure.

```python
repo//foo:bin buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin
repo//foo:bin?opt buck-out/v2/gen/repo/b706492dec65e54c/foo/bin
```

This preserves modifiers in the exact same way that is specified from the CLI invocation, which allows users to differentiate which path belongs to dev modifier and which path belongs to opt modifier, without needing to understand very much about modifiers.

## Build Report

Current build report for `buck2 build repo//foo:bin` looks as follows. For readability, we will skip irrelevant fields.

```python
{
  "results": {
    "repo//foo:bin": {
      "success": "SUCCESS",
      "outputs": {
        "DEFAULT": [
          "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
        ]
      },
      "other_outputs": {},
      "configured": {
        "cfg#57b1cdd23074b8c3": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
            ]
          },
          "other_outputs": {}
        }
      },
      "errors": []
    }
  },
  # other fields
}
```

When `?`-syntax is used, we will also preserve the modifiers in the build report results key. For example, this is what the build report looks like with `buck2 build repo//foo:bin repo//foo:bin?opt`.

```python
{
  "results": {
    "repo//foo:bin": {
      "success": "SUCCESS",
      "outputs": {
        "DEFAULT": [
          "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
        ]
      },
      "other_outputs": {},
      "configured": {
        "cfg#57b1cdd23074b8c3": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
            ]
          },
          "other_outputs": {}
        }
      },
      "errors": [],
      "target_label": "repo//foo:bin"
    },
    "repo//foo:bin?opt": {
      "success": "SUCCESS",
      "outputs": {
        "DEFAULT": [
          "buck-out/v2/gen/repo/b706492dec65e54c/foo/bin"
        ]
      },
      "other_outputs": {},
      "configured": {
        "cfg#b706492dec65e54c": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/b706492dec65e54c/foo/bin"
            ]
          },
          "other_outputs": {}
        }
      },
      "errors": [],
      "target_label": "repo//foo:bin"
    }
  },
  # other fields
}
```

The keys in `results` are `repo//foo:bin` and `repo//foo:bin?opt`. Since `repo//foo:bin?opt` is not a proper target label, we add a key called “target_label” which will display the target label without any modifiers (in this case `repo//foo:bin`). We will also add a ?`modifiers` key that will resolve to a list of all modifiers applied after `?`.

### Alternate Design

A possible alternate design is that we add a `per_target_modifiers` section of the build report similar to the `configured` section.

```python
{
  "results": {
    "repo//foo:bin": {
      "success": "SUCCESS",
      "outputs": {
        "DEFAULT": [
          "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
        ]
      },
      "other_outputs": {},
      "configured": {
        "cfg#57b1cdd23074b8c3": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
            ]
          },
          "other_outputs": {}
        },
        "cfg#b706492dec65e54c": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/b706492dec65e54c/foo/bin"
            ]
          },
          "other_outputs": {}
        }
      },
      "errors": [],
      "modifiers": {
        "null": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/57b1cdd23074b8c3/foo/bin"
            ]
          },
          "other_outputs": {}
        },
        "opt": {
          "errors": [],
          "success": "SUCCESS",
          "outputs": {
            "DEFAULT": [
              "buck-out/v2/gen/repo/b706492dec65e54c/foo/bin"
            ]
          },
          "other_outputs": {}
        }
      },
    }
  },
  # other fields
}
```

The main reason to prefer approach #1 is that it better accommodates tools that wrap a user’s buck build invocation and do extra processing on the build report. With approach #2, if a user passes in `repo//foo:bin?opt` as the target pattern to build, then the wrapper tool needs to understand that `?opt` specifies a modifier and that it needs to do a string split on `?` to find the correct section of the build report. With approach #1, it can look up `repo//foo:bin?opt` directly from the build report without understanding that a modifier was used.

The benefit of approach #2 is that the `configured` section looks more understandable in this approach than the previous approach, and in general it leads to a shorter build report.

## Target Universe

A [target universe](../../concepts/glossary.md#target-universe) is a set of configured targets and their transitive deps that Buck looks up from to resolve unconfigured target labels. For example, `buck2 build repo//lib:singleton --target-universe=repo//foo:bin` will build all configured variants of `repo//lib:singleton` in transitive deps of `repo//foo:bin`. This section applies to all commands that can explicitly use the `--target-universe` flag like `audit providers` and `aquery`.

### Explicit target universe

If `--target-universe` flag is specified on CLI, then `?` can only be used in `--target-universe` flag. In other words,

- `buck2 build repo//lib:singleton --target-universe=repo//foo:bin?asan` is allowed.
- Likewise, `buck2 build repo//lib:singleton --target-universe=repo//foo:bin+repo//foo:bin?asan` is allowed.
- `buck2 build repo//lib:singleton?asan --target-universe=repo//foo:bin?asan` is not allowed.
- Likewise, `buck2 build repo//lib:singleton?asan --target-universe=repo//foo:bin` is also not allowed.

The above examples all look at builds, but the same principles apply to other commands that can use `--target-universe` like `audit providers` and `cquery`.

This behavior also roughly matches how `--modifier` flag works with `--target-universe` today, which is that `--modifier` is only applied to resolve the target universe, and not the target patterns specified.

This is probably not the most ideal behavior, but it is probably the more restrictive behavior, meaning it will be easier for us to change this behavior in the future if we find a use case that needs it.

## Cquery

In cquery, ?-syntax will *only* be allowed in `--target-universe`. This means that

- `buck2 cquery repo//lib:singleton --target-universe=repo//foo:bin?asan` is allowed
- `buck2 cquery repo//lib:singleton?asan –target-universe=repo//foo:bin?asan` and `buck2 cquery repo//lib:singleton?asan --target-universe=repo//foo:bin` are disallowed.
- Additionally, `buck2 cquery repo//lib:singleton?asan` is *disallowed*. The reason for this is that cquery [infers a target universe](../../bxl/explanation/bxl_cquery_vs_cli_cquery.md#cli-buck2-cquery) from all target literals specified in the query when explicit `--target-universe` is not specified. Thus `buck2 cquery repo//lib:singleton?asan` naturally expands to `buck2 cquery repo//lib:singleton?asan --target-universe=repo//lib:singleton?asan`.

### Possible relaxation

Not being able to specify `?` in cquery outside of `--target-universe` is rather unintuitive behavior, and it’s ironic that a `cquery` command cannot easily customize configurations of targets. It’s possible that we may allow  `buck2 cquery repo//lib:singleton?asan` and by extension `buck2 cquery repo//lib:singleton?asan --target-universe=repo//lib:singleton?asan` in the future.

One possible relaxation is that if a literal in a query expression is specified with a `?modifier`, then that target literal will resolve in the target universe according to its modifiers applied instead of matching configured targets with the same unconfigured target label in the target universe.

Take the example of `buck2 cquery set(repo//foo:bin repo//lib:singleton)`. This will print multiple variants of `repo//lib:singleton` in different configurations because `repo//lib:singleton` shows up in deps of `repo//foo:bin` and will be configured once in its default configuration. Let’s assume the output looks something like this.

```python
repo//foo:bin (cfg1)
repo//lib:singleton (cfg1)
repo//lib:singleton (cfg2)
```

If a user invokes `buck2 cquery set(repo//foo:bin repo//lib:singleton?asan)`, then `repo//lib:singleton?asan` will not resolve to any copies of `repo//lib:singleton` in deps of `repo//foo:bin`, so output will look like this.

```python
repo//foo:bin (cfg1)
repo//lib:singleton (cfg3)
```

Note that all literals with modifiers applied still go through target universe resolution. If a user invokes `buck2 cquery “set(repo//foo:bin?asan repo//lib:singleton)”`, then `repo//lib:singleton` will continue to be resolved in the target universe of `repo//foo:bin?asan`, so the output will look like

```python
repo//foo:bin (cfg4)
repo//lib:singleton (cfg2)
repo//lib:singleton (cfg4)
```

To list the exact behavior in this scenario,

- `buck2 cquery repo//lib:singleton?asan --target-universe=repo//lib:singleton?asan` is allowed.
- `buck2 cquery repo//lib:singleton?asan –target-universe=repo//foo:bin` is not allowed because `repo//lib:singleton?asan` is not directly specified in the target universe.

We may consider relaxing to this behavior in the future if there are demands for it.

### Another possible relaxation

Additionally, it’s possible that we allow `buck2 cquery repo//lib:singleton?asan –target-universe=repo//foo:bin `to resolve `repo//lib:singleton` to its configuration with asan applied possibly outside of target universe of `repo//foo:bin` in the future. Unfortunately, this is unintuitive in a couple ways.

- `repo//lib:singleton?asan` likely resolves to a configured target outside of the target universe of `repo//foo:bin?asan`. If we were to respect `asan` modifier when resolving `repo//lib:singleton`, then the build command will build nothing. If we don’t respect `asan` modifier on `repo//lib:singleton`, then we will be ignoring the modifiers specified and building every copy of `repo//lib:singleton` that shows up in deps of `repo//foo:bin`. Neither behavior would be intuitive for the average user.
- With :`foo?modifiers`, Buck will actively configure `:foo` outside of the target universe. With just `:foo`, buck will not attempt to do that. This behavior is inconsistent and a bit unintuitive.

## Commands that receive unconfigured targets

Commands that operate on unconfigured targets like `utargets` and `uquery` should error when `?modifier` is specified. This can be relaxed in the future if it is found that it is better to ignore modifiers instead.

## BXL

By default, passing a target with `?modifier` to `cli_args.target_label()` should be an error.

We can introduce a `cli_args.configured_target_label()` to allow passing in configured target label with `?modifier`.

## Testing

Testing should understand `repo//foo:test?opt+asan` as target `repo//foo:test` with `opt` and `asan` modifiers applied, similar to `repo//foo:test -m opt -m asan`.
