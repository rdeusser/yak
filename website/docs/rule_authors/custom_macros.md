---
id: custom_macros
title: Macros
---

# Macros

It is possible to define Starlark functions that have the side-effect of
creating build targets. Such functions are called _macros_.

## On this page

- [How to define a macro](#defining)
- [Compound build targets: macros that expand to multiple targets](#compound-targets)
- [How to view expanded macros](#viewing)

## How to define a macro {#defining}

We require that you define and maintain your macros in files that are external
to your build files. These files must have an extension; we recommend that you
use the extension, `.bzl`.

To make your macros accessible to a build file, import them using the `load()`
function.

In the following example, the macro `python_library_using_requests`, defined
in the file `python_macros.bzl`, invokes a macro named `python_library` that
depends on the Requests library.

**`python_macros.bzl`**

```python
def python_library_using_requests(
    name,
    srcs=[],
    resources=[],
    deps=[],
    visibility=[]):
  python_library(
    name = name,
    srcs = srcs,
    resources = resources,
    deps = [
      # This assumes this is where Requests is in your project.
      '//third_party/python/requests:requests',
    ] + deps,
    visibility = visibility,
  )
```

Instantiating this macro looks the same as defining a built-in build rule. In
the following code, we assume that `python_macros.bzl` is stored in the
subdirectory `libs/python_libs/team_macros`.

```python
#
# load the macro from the external file
#
load("//libs/python_libs/team_macros:python_macros.bzl", "python_library_using_requests")

#
# Calling this function has the side-effect of creating
# a python_library() rule named 'util' that depends on Requests.
#
python_library_using_requests(
  name = 'util',
  # Source code that depends on Requests.
  srcs = glob(['*.py']),
)
```

## Compound build rules: macros that expand to multiple rules {#compound-targets}

:::caution

While this is supported we strongly recommend you instead write user
defined rules since complex macros have performance and maintainability
drawbacks.

:::

You can also create more sophisticated macros that expand into multiple build
rules. For example, you could create a macro that produces targets for both
debug and release versions of a binary:

```python
def create_binaries(
    name,
    srcs,
    debug_flags,
    release_flags,
    deps):

  # This loop will create two cxx_binary rules.
  for type in [ 'debug', 'release' ]:
    # Select the appropriate compiler flags.
    if type == 'debug':
      flags = debug_flags
    else:
      flags = release_flags

    cxx_binary(
      # Note how we must parameterize the name of the
      # target so that we avoid creating two build
      # targets with the same name.
      name = '%s_%s' % (name, type),
      srcs = srcs,
      compiler_flags = flags,
      deps = deps,
      visibility = [
        'PUBLIC',
      ],
    )
```

As in the previous example, instantiating this macro _looks_ the same as
specifying a single rule:

```python
create_binaries(
  name = 'chat',
  srcs = ['main.cpp'],
  debug_flags = ['-O0', '-g'],
  release_flags = ['-O2'],
  deps = [
    # ...
  ],
)
```

However, instantiating this macro actually creates _two_ targets. For example,
if you instantiated this macro in the build file, `apps/chat/YAK`, it
would create the following rules:

```
//apps/chat:chat_debug
//apps/chat:chat_release
```

Note, though, that in this scenario, the following is **NOT** a target:

```
//apps/chat:chat                        # MACRO, NOT A TARGET
```

Therefore, the following commands do not work, which could be confusing for
developers who don't realize that `chat` is a macro rather than a target.

```
yak build //apps/chat:chat              # FAILS
yak targets --type create_binaries      # FAILS
```

## How to view expanded macros {#viewing}

Use `yak targets` to view the resulting targets after expanding all macros. The
following invocation of `yak targets` show the resulting targets from the
preceding example, but not the macro that created them.

```
yak targets //apps/chat/...
```
