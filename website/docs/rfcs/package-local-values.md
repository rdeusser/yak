# Package-local values

This RFC proposes to extend yak Starlark with package-local values.

## Why

Developer infrastructure teams want to have some per-directory configuration
files, accessible from Starlark macros.

For example, a project NNN may want to switch to building using LLVM 15 by
default. End users would want to have an easy instruction how to do that, after
developer infrastructure teams provided instructions and infrastructure for
that.

## What we have now

Currently, a large monorepo can use a `get_modes` mechanism.

`get_modes` symbol is registered in per-package implicit symbols.

This symbol can be accessed from macros using `implicit_package_symbol`
function.

`get_modes` functions are package-local, but all `BUILD_MODE.bzl` files need to
be registered in global buckconfig, which is not ideal.

Proposed per-package properties can replace `get_modes` mechanism.

## API

### `PACKAGE` files

Before evaluating `YAK` file, yak will evaluate all `PACKAGE` files in the
same directory and all parent directories. Absent `PACKAGE` files are treated as
empty files.

All relevant `PACKAGE` files are executed sequentially from the root directory
to the current directory (but unrelated `PACKAGE` files can be executed in
parallel). Evaluating `PACKAGE` files sequentially provides additional
guarantees, for example, attempt to override a property (unless explicitly
requested) should fail with Starlark call stack.

Each `PACKAGE` file is evaluated at most once (like `bzl` file).

`PACKAGE` files may load arbitrary `bzl` files. `YAK`-specific functions called
in `bzl` files (like rule functions) are available, but calling functions from
`PACKAGE` files is an error. This way, `bzl` files are evaluated only once
regardless of whether they are loaded from `PACKAGE` or `YAK` file.

### API

`PACKAGE` files have a global function:

#### `PACKAGE` file API

```python
def write_package_value(
    name: str,
    value: "",
    overwrite: bool = False,
): ...
```

Name is a string which must contain exactly one dot symbol (just to enforce code
style).

Value is an arbitrary Starlark value, for example, an integer, a list of
integer, a struct or a function.

When `overwrite` is `False` (default), attempt to overwrite per-package value
defined in parent `PACKAGE` file will fail.

Written values are frozen when `PACKAGE` file evaluation is finished.

Note `write_package_value` symbol exists in `bzl` globals, and it can be called
from `bzl` file in context of `PACKAGE` evaluation, but calling
`write_package_file` is an error on context of `YAK` evaluation.

Modifying `PACKAGE` file logically invalidates the `YAK` file of this package,
and all `PACKAGE` and `YAK` files of subpackages. However, `YAK` file
evaluation may track which package-local values were accessed and only
invalidate `YAK` files which were potentially affected (similarly to how we do
it with buckconfigs, with individual properties being projection keys).

#### `YAK` file API

`YAK` files (and `bzl` files included from `YAK` files) have a global
function:

```python
def read_package_value(
    name: str,
): ...
```

This function returns the nearest value registered per package, or `None` is
such value does not exist.

This function is available in `bzl` files, but attempt to call this function in
context of `PACKAGE` file evaluation results in an error. This restriction can
be lifted in the future.

Per-package values are **not** accessible as global symbols in `YAK` files. We
may reconsider it in the future.

### `read_config`

`PACKAGE` files may call `read_config` function.
