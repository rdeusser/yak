---
id: build_target
title: Build Target
---

# Build Target

A _build target_ is a string that identifies a build target in your project.
Build targets are used as arguments to yak commands, such as
[`yak build`](../../users/commands/build) and
[`yak run`](../../users/commands/run). Build targets are also used as
arguments to [build rules](build_rule.md) to enable one target to reference
another. For example, a build rule might use a build target to reference another
target in order to specify that target as a _dependency_.

#### Fully-qualified build targets

Here is an example of a _fully-qualified_ build target:

```
cell//java/com/example/share:ui
```

A fully-qualified build target has three components:

1. The `cell//` prefix indicates that the subsequent path is from the _root_ of
   `cell`.
2. The `java/com/example/share` between the `//` prefix and the colon (`:`)
   indicates that the [build file](build_file.md) (usually named `YAK`) is
   located in the directory `java/com/example/share`.
3. The `ui` after the colon (`:`) indicates the name of the build target within
   the build file. Build target names must be unique within a build file. By
   _name_ we mean, more formally, the value of the `name` argument to the build
   rule.

Note that the name of the build file itself—usually YAK—does _not_ occur in the
build target. All build files within a given yak project must have the same
name—defined in the `[buildfile].name` entry of `.yakconfig`. Therefore, it is
unnecessary to include the name in the target. The full regular expression for a
fully-qualified build target is as follows:

```
[A-Za-z0-9._-]*//[A-Za-z0-9/._-]*:[A-Za-z0-9_/.=,@~+-]+
|- cell name -|  | package path | |--- target name ----|
```

In yak, a _cell_ defines a directory tree of one or more yak packages. For
more information about yak cells and their relationship to packages and
projects, see the [Key Concepts](key_concepts.md) topic. **NOTE:** All target
paths are assumed to start from the root of the yak project. yak does not
support specifying a target path that starts from a directory below the root.
Although the double forward slash (`//`) that prefixes target paths can be
omitted when specifying a target from the command line (see **Pro Tips** below),
yak still assumes that the path is from the root. yak does support
_relative_ build paths, but in yak, that concept refers to specifying build
targets _from within_ a build file. See **Relative build targets** below for
more details.

#### Cell relative build targets

A _cell relative_ build target omits the cell, and is inferred to be relative to
the current cell.

#### Package relative build targets

A _package relative_ build target can be used to reference a build target
_within the same _[_build file_](build_file.md) (aka _package_). A relative
build target starts with a colon (`:`) and is followed by only the third
component (or _short name_) of the fully-qualified build target. The following
snippet from a build file shows an example of using a relative path.

```python
## Assume this target is in //java/com/example/share/YAK#
cxx_binary(
  name = 'ui_app',
  deps = [
    ## The following target path
    ##   //java/com/example/share:ui
    ## is the same as using the following relative path.#
    ':ui',
  ],
)
```

#### Targets named after their package's directory

A build target whose name is the last component of its package path can omit
the name, so `//lib/greeting` names `//lib/greeting:greeting`. This holds in
build files, `.bzl` files, and on the command line. Setting
`[yak] infer_target_names = false` in `.yakconfig` makes the short form an error
in build and `.bzl` files.

## Command-line Pro Tips

Here are some ways that you can reduce your typing when you specify build
targets as command-line arguments to the `yak build` or `yak run` commands.
Consider the following example of a fully-qualified build target used with the
`yak build` command:

```sh
yak build cell//java/com/example/share:share
```

Although yak is always strict when parsing build targets in build files, yak
is flexible when parsing build targets on the command-line. Specifically, the
leading `//` is optional on the command line, so the above could be:

```sh
yak build java/com/example/share:share
```

Also, if there is a forward slash before the colon, it is ignored, so this could
also be written as:

```sh
yak build java/com/example/share/:share
```

which enables you to produce the red text shown below using tab-completion,
which dramatically reduces how much you need to type:

```sh
yak build java/com/example/share/:share
```

Finally, if the final path element matches the value specified after the colon,
it can be omitted:

```sh
# This is treated as //java/com/example/share:share.
yak build java/com/example/share/
```

which makes the build target even easier to tab-complete. For this reason, the
name of the build target for the primary deliverable in a build file is often
named the same as the parent directory. That way, it can be built from the
command-line with less typing.

## See also

yak supports the ability to define **_aliases_ for build targets**; using
aliases can improve brevity when specifying targets on the yak command line.
For more information, see the [`[alias]`](yakconfig.md#alias) section in the
documentation for [`.yakconfig`](yakconfig.md). A
[**build target pattern**](target_pattern.md) is a string that describes a set
of one or more build targets. For example, the pattern `//...` is used to build
an entire project. For more information, see the **Build Target Pattern** topic.
