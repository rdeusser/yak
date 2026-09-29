---
id: alias
title: Alias
---

The `alias` rule creates another name by which an existing rule can be referred
to. The [configured_alias](#configured_alias) variant also sets the platform to
build the target with.

## alias

The `alias` rule has the following relevant attributes:

- `name` - (required) what the `actual`'s label should be aliased as.
- `actual` - (required) a target label.

**Example**

```python
filegroup(
    name = "foo",
    srcs = ["foo.txt"],
)

alias(
    name = "other_foo",
    actual = ":foo",
)
```

## configured_alias

The `configured_alias` rule has the following relevant attributes:

- `name` - (required) what the `actual`'s label should be aliased as.
- `configured_actual` - a configured label (mapped to a configured dep under the
  hood so the providers can be simply forwarded).
- `fallback_actual` - if `configured_actual` is not set, then fallback to this
  value, which is an unconfigured dep. If `configured_actual` is not set, then
  `fallback_actual` must be set.
- `platform` - the platform to build the aliased target with.

Outside of simply pointing at another target, this target has one other useful
feature - it contains a platform argument.

This makes the alias rule useful for two distinct scenarios:

- **Configuration switching during the build**. For example, there is an iOS
  target that needs to build a dependency for WatchOS so it can include it in
  the bundle. This can be represented by the iOS target having a dependency on
  an alias of the Watch app with `platform = "//the/desired/watchos:platform"`.
- **Using a target to refer to another in a non-standard configuration**. For
  example, if you want to have an experimental version of an app, you could
  represent that as an alias with an 'experimental' configuration pointing to
  the original target.

**Example**

```Python
configured_alias(
    name = "foo-with-platform1",
    actual = "//lib:foo",
    platform = "//some_config:platform1",
    visibility = ["PUBLIC"],
)
```
