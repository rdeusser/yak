# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Combine the attributes we generate, we the custom implementations we have.
load("@prelude//:rules_impl.bzl", "categorized_extra_attributes", "categorized_rule_decl_records", "extra_implemented_rules", "toolchain_rule_names")
load("@prelude//apple:apple_platforms.bzl", "APPLE_PLATFORMS_KEY")
load("@prelude//configurations:rules.bzl", _config_implemented_rules = "implemented_rules")
load("@prelude//decls:common.bzl", "prelude_rule")

def _unimplemented(name, ctx):
    fail("Unimplemented rule type `{}` for target `{}`.".format(name, ctx.label))

def _unimplemented_impl(name):
    # We could use a lambda here, but then it means every single parse evaluates a lambda.
    # Lambda's have tricky semantics, so using partial lets us test Starlark prototypes with
    # some features disabled.
    return partial(_unimplemented, name)

def _mk_rule(rule_spec: typing.Any, extra_attrs: dict[str, typing.Any] = dict(), impl_override: [typing.Callable, None] = None, **kwargs):
    name = rule_spec.name

    # copy so we don't try change the passed in object
    attributes = dict(rule_spec.attrs)
    attributes.update(extra_attrs)

    # Add _apple_platforms to all rules so that we may query the target platform to use until we support configuration
    # modifiers and can use them to set the configuration to use for operations.
    # Map of string identifier to platform.
    attributes[APPLE_PLATFORMS_KEY] = attrs.dict(key = attrs.string(), value = attrs.dep(), sorted = False, default = {})

    cfg = rule_spec.cfg

    if cfg != None:
        kwargs["cfg"] = cfg

    if rule_spec.docs:
        doc = rule_spec.docs

        # This is awkward. When generating documentation, we'll strip leading whitespace
        # like it's a python docstring. For that to work here, we need the "Examples:" line
        # to match the other lines for leading whitespace. We've just hardcoded this to
        # be what its expected to be in prelude.
        # TODO(cjhopman): Figure out something better here.
        if rule_spec.examples:
            # NOTE: DocString needs 'Examples:' to render the sections properly, that code should be updated along with this
            doc += "\n{}Examples:\n{}".format(" " * 8, rule_spec.examples)
        if rule_spec.further:
            doc += "\n{}Additional notes:\n{}".format(" " * 8, rule_spec.further)

        kwargs["doc"] = doc

    impl = rule_spec.impl
    extra_impl = getattr(extra_implemented_rules, name, None)
    if extra_impl:
        if impl:
            fail("{} had an impl in the declaration and in the extra_implemented_rules overrides".format(name))
        impl = extra_impl
    if not impl:
        impl = _unimplemented_impl(name)
    if impl_override != None:
        impl = impl_override
    if rule_spec.uses_plugins != None:
        kwargs["uses_plugins"] = rule_spec.uses_plugins
    if rule_spec.supports_incoming_transition != None:
        kwargs["supports_incoming_transition"] = rule_spec.supports_incoming_transition

    is_toolchain_rule = rule_spec.is_toolchain_rule
    is_toolchain_rule_via_rule_name = name in toolchain_rule_names
    if is_toolchain_rule == None:
        is_toolchain_rule = is_toolchain_rule_via_rule_name
    elif is_toolchain_rule_via_rule_name:
        fail("Cannot set `is_toolchain_rule` on `prelude_rule` and also via `toolchain_rule_names`")

    kwargs.setdefault("is_configuration_rule", name in _config_implemented_rules)
    kwargs.setdefault("is_toolchain_rule", is_toolchain_rule)
    return rule(impl = impl, attrs = attributes, **kwargs)

def _categorized_decls():
    grouped_decls = {}
    for group_name, decl_set in categorized_rule_decl_records.items():
        group_rules = {}
        for rule in dir(decl_set):
            group_rules[rule] = getattr(decl_set, rule)
        grouped_decls[group_name] = group_rules
    return grouped_decls

Attributes = dict[str, Attr]

def _update_categorized_rules(categorized_rules: dict[str, dict[str, prelude_rule]], categorized_extra_attributes: dict[str, dict[str, Attributes]]):
    flatten_rules = {}
    for category, rules in categorized_rules.items():
        for rule_name, rule_spec in rules.items():
            flatten_rules[rule_name] = (category, rule_spec)
    for attr_category, extra_attributes_dict in categorized_extra_attributes.items():
        for rule_name, extra_attributes in extra_attributes_dict.items():
            if rule_name in flatten_rules:
                category, rule_spec = flatten_rules[rule_name]
                d = dict(rule_spec.attrs)
                d.update(extra_attributes)

                # Use the category from the categorized_rules not categorized_extra_attributes
                categorized_rules[category][rule_name] = prelude_rule(
                    name = rule_spec.name,
                    impl = rule_spec.impl,
                    attrs = d,
                    docs = rule_spec.docs,
                    examples = rule_spec.examples,
                    further = rule_spec.further,
                    uses_plugins = rule_spec.uses_plugins,
                    supports_incoming_transition = rule_spec.supports_incoming_transition,
                    is_toolchain_rule = rule_spec.is_toolchain_rule,
                    cfg = rule_spec.cfg,
                )
            else:
                if attr_category not in categorized_rules:
                    categorized_rules[attr_category] = {}
                categorized_rules[attr_category][rule_name] = prelude_rule(
                    name = rule_name,
                    impl = None,
                    attrs = extra_attributes,
                    docs = None,
                    examples = None,
                    further = None,
                    uses_plugins = None,
                    supports_incoming_transition = None,
                )

_categorized_declared_rules = _categorized_decls()
_update_categorized_rules(_categorized_declared_rules, categorized_extra_attributes)

_declared_rules = {rule_name: rule for category in _categorized_declared_rules.values() for rule_name, rule in category.items()}

categorized_rules = {
    group_name: {rule_name: _mk_rule(rule_decl) for rule_name, rule_decl in group_rules.items()}
    for group_name, group_rules in _categorized_declared_rules.items()
}
rules = {rule_name: rule for category in categorized_rules.values() for rule_name, rule in category.items()}

# The rules are accessed by doing module.name, so we have to put them on the correct module.
load_symbols(rules)

# TODO(akrieger): Remove this and instead refactor to allow impl bzl files to export attrs.
def clone_rule(rule: str, extra_attrs: dict[str, typing.Any] = dict(), impl_override = None, **kwargs):
    if not rule in _declared_rules:
        fail("Tried clone rule {} which does not exist".format(rule))
    return _mk_rule(_declared_rules[rule], extra_attrs, impl_override, **kwargs)
