# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


# Add a try except to force eager importing
try:
    from _static_extension_utils import _check_module, StaticExtensionLoader
except BaseException:
    raise


class StaticExtensionFinder:
    ModuleSpec = None

    @classmethod
    def find_spec(cls, fullname, path, target=None):
        """
        Use fullname to look up the PyInit function in the main binary. Returns None if not present.
        This allows importing CExtensions that have been statically linked in.
        """

        if not fullname:
            return None
        if not _check_module(fullname):
            return None
        spec = cls.ModuleSpec(
            fullname, StaticExtensionLoader, origin="static-extension", is_package=False
        )
        return spec


def _initialize() -> None:
    # The imports are here to avoid circular dependencies.
    import sys
    from importlib.machinery import ModuleSpec

    StaticExtensionFinder.ModuleSpec = ModuleSpec

    sys.meta_path.insert(0, StaticExtensionFinder)
