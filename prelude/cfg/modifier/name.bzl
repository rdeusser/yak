# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# This is an ordered dictionary of constraint setting target to an optional transform.
# The constraint settings listed below are used to name the configuration, where the first
# constraint settings are named first in the configuration. The transform, if specified,
# can change how the name appears in the configuration. By default, if there is no transform,
# we just use name of the constraint value in the configuration name. If transform is
# specified, the transform will be applied on the existing constraint value name to return a
# new string to be used in the configuration.
# The table is empty, so every configuration that modifiers generate is named
# `cfg:<empty>`.
# TODO: Name configurations after the prelude's constraint settings, such as
# `prelude//os/constraints:os` and `prelude//cpu/constraints:cpu`.
NAMED_CONSTRAINT_SETTINGS = {}

# Mark all modifier generated configurations with a `cfg:` prefix, so that they
# are easy to recognize.
_CFG_PREFIX = "cfg:"
_EMPTY_CFG_NAME = _CFG_PREFIX + "<empty>"

def cfg_name(cfg: ConfigurationInfo) -> str:
    """Derives a reasonable name for a ConfigurationInfo"""

    name_list = []
    constraints = {str(key): value for key, value in cfg.constraints.items()}
    for constraint_setting, transform in NAMED_CONSTRAINT_SETTINGS.items():
        constraint_name = None
        if constraint_setting in constraints:
            constraint_value_label = constraints[constraint_setting].label
            if transform:
                constraint_name = transform(constraint_value_label)
            else:
                constraint_name = str(constraint_value_label.name)
        if constraint_name:
            name_list.append(constraint_name)

    if len(name_list) == 0:
        name = _EMPTY_CFG_NAME
    else:
        name = _CFG_PREFIX + "-".join(name_list)
    return name
