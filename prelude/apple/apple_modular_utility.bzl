# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# The module cache has a fixed location, so Xcode and LLDB can share it.
#
# TODO: This needs to be changed to use $TMPDIR in a
# wrapper for modular clang compilation.
MODULE_CACHE_PATH = "/tmp/buck-module-cache"
