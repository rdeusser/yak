# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# `native` is fine to use in the prelude for v2

# This is yak's shim import. Any public symbols here will be available within
# **all** interpreted files.

load("@prelude//:rules.bzl", __rules__ = "rules")
load(
    "@prelude//apple:apple_macro_layer.bzl",
    "apple_binary_macro_impl",
    "apple_bundle_macro_impl",
    "apple_library_for_distribution_macro_impl",
    "apple_library_macro_impl",
    "apple_metal_library_macro_impl",
    "apple_package_macro_impl",
    "apple_test_macro_impl",
    "apple_universal_executable_macro_impl",
    "apple_xcuitest_macro_impl",
    "prebuilt_apple_framework_macro_impl",
)
load("@prelude//apple:prebuilt_apple_xcframework_macro_impl.bzl", "prebuilt_apple_xcframework_macro_impl")
load("@prelude//apple/swift:swift_toolchain_macro_layer.bzl", "swift_toolchain_macro_impl")
load("@prelude//cxx:cxx_toolchain.bzl", "cxx_toolchain_inheriting_target_platform")
load("@prelude//cxx:cxx_toolchain_macro_layer.bzl", "cxx_toolchain_macro_impl")
load("@prelude//cxx:cxx_toolchain_types.bzl", _cxx = "cxx")
load("@prelude//erlang:erlang.bzl", _erlang_application = "erlang_application", _erlang_tests = "erlang_tests")
load("@prelude//python:toolchain.bzl", _python = "python")
load("@prelude//rust:link_info.bzl", "RustLinkInfo")
load("@prelude//rust:rust_common.bzl", "rust_common_macro_wrapper")
load("@prelude//rust:rust_library.bzl", "rust_library_macro_wrapper")
load("@prelude//rust:sources.bzl", "RustSources")
load("@prelude//rust:with_workspace.bzl", "with_rust_workspace")
load("@prelude//user:all.bzl", _user_rules = "rules")
load(
    "@prelude//utils:buckconfig.bzl",
    _read_config = "read_config_with_logging",
    _read_root_config = "read_root_config_with_logging",
    log_buckconfigs = "LOG_BUCKCONFIGS",
)
load("@prelude//utils:selects.bzl", "selects")

def __struct_to_dict(s):
    vals = {}
    for name in dir(s):
        vals[name] = getattr(s, name)
    return vals

# export_file src defaults to name, despite being string vs source, so adjust it in the macros
def _export_file_macro_stub(name, src = None, **kwargs):
    __rules__["export_file"](name = name, src = name if src == None else src, **kwargs)

def _configured_alias_macro_stub(
    name,
    actual,
    platform,
    # Whether to fallback to a unconfigured `alias` if `platform` is `None`.
    fallback_to_unconfigured_alias = False,
    **kwargs,
):
    pred = lambda platform: platform != None or not fallback_to_unconfigured_alias
    __rules__["configured_alias"](
        name = name,
        # `actual` needs to be a pair of target + platform, as that's the format
        # expected by the `configured_dep()` field
        # Use a select map to make this thing `None` if `platform` is `None`.
        configured_actual = selects.apply(
            platform,
            lambda platform: (actual, platform) if pred(platform) else None,
        ),
        # Make sure that exactly one of `configured_actual` or `fallback_actual` is set
        fallback_actual = selects.apply(
            platform,
            lambda platform: None if pred(platform) else actual,
        ),
        platform = platform,
        **kwargs,
    )

def _apple_bundle_macro_stub(**kwargs):
    apple_bundle_macro_impl(apple_bundle_rule = __rules__["apple_bundle"], apple_resource_bundle_rule = __rules__["apple_resource_bundle"], **kwargs)

def _apple_watchos_bundle_macro_stub(**kwargs):
    apple_bundle_macro_impl(apple_bundle_rule = __rules__["apple_watchos_bundle"], apple_resource_bundle_rule = __rules__["apple_resource_bundle"], **kwargs)

def _apple_macos_bundle_macro_stub(**kwargs):
    apple_bundle_macro_impl(apple_bundle_rule = __rules__["apple_macos_bundle"], apple_resource_bundle_rule = __rules__["apple_resource_bundle"], **kwargs)

def _apple_test_macro_stub(**kwargs):
    apple_test_macro_impl(apple_test_rule = __rules__["apple_test"], apple_resource_bundle_rule = __rules__["apple_resource_bundle"], **kwargs)

def _apple_xcuitest_macro_stub(**kwargs):
    apple_xcuitest_macro_impl(apple_xcuitest_rule = __rules__["apple_xcuitest"], **kwargs)

def _apple_binary_macro_stub(**kwargs):
    apple_binary_macro_impl(apple_binary_rule = __rules__["apple_binary"], apple_universal_executable = __rules__["apple_universal_executable"], **kwargs)

def _apple_library_macro_stub(**kwargs):
    apple_library_macro_impl(apple_library_rule = __rules__["apple_library"], **kwargs)

def _apple_metal_library_macro_stub(**kwargs):
    apple_metal_library_macro_impl(apple_metal_library_rule = __rules__["apple_metal_library"], **kwargs)

def _apple_library_for_distribution_macro_stub(**kwargs):
    apple_library_for_distribution_macro_impl(apple_library_for_distribution_rule = __rules__["apple_library_for_distribution"], **kwargs)

def _apple_package_macro_stub(**kwargs):
    apple_package_macro_impl(apple_package_rule = __rules__["apple_package"], apple_ipa_package_rule = __rules__["apple_ipa_package"], **kwargs)

def _apple_universal_executable_macro_stub(**kwargs):
    apple_universal_executable_macro_impl(apple_universal_executable_rule = __rules__["apple_universal_executable"], **kwargs)

def _swift_toolchain_macro_stub(**kwargs):
    rule = __rules__["swift_toolchain"]

    swift_toolchain_macro_impl(swift_toolchain_rule = rule, **kwargs)

def _cxx_toolchain_macro_stub(**kwargs):
    cxx_toolchain_macro_impl(cxx_toolchain_rule = cxx_toolchain_inheriting_target_platform, **kwargs)

def _cxx_toolchain_override_macro_stub(**kwargs):
    cxx_toolchain_macro_impl(cxx_toolchain_rule = _user_rules["cxx_toolchain_override"], **kwargs)

def _erlang_application_macro_stub(**kwargs):
    _erlang_application(erlang_app_rule = __rules__["erlang_app"], erlang_app_includes_rule = __rules__["erlang_app_includes"], **kwargs)

def _erlang_tests_macro_stub(**kwargs):
    _erlang_tests(erlang_app_rule = __rules__["erlang_app"], erlang_test_rule = __rules__["erlang_test"], **kwargs)

def _rust_library_macro_stub(**kwargs):
    rust_library = rust_common_macro_wrapper(__rules__["rust_library"])
    rust_library = rust_library_macro_wrapper(rust_library)
    rust_library(**kwargs)

def _rust_binary_macro_stub(**kwargs):
    rust_binary = rust_common_macro_wrapper(__rules__["rust_binary"])
    rust_binary(**kwargs)

def _rust_test_macro_stub(**kwargs):
    rust_test = rust_common_macro_wrapper(__rules__["rust_test"])
    rust_test(**kwargs)

def _prebuilt_apple_framework_macro_stub(**kwargs):
    prebuilt_apple_framework_macro_impl(prebuilt_apple_framework_rule = __rules__["prebuilt_apple_framework"], **kwargs)

def _prebuilt_apple_xcframework_macro_stub(**kwargs):
    prebuilt_apple_xcframework_macro_impl(
        filegroup_rule = __rules__["filegroup"], genrule = __rules__["genrule"], prebuilt_apple_framework_rule = __rules__["prebuilt_apple_framework"], **kwargs
    )

# TODO(cjhopman): These macro wrappers should be handled in prelude/rules.bzl+rule_impl.bzl.
# Probably good if they were defined to take in the base rule that
# they are wrapping and return the wrapped one.
__extra_rules__ = {
    "apple_binary": _apple_binary_macro_stub,
    "apple_bundle": _apple_bundle_macro_stub,
    "apple_library": _apple_library_macro_stub,
    "apple_library_for_distribution": _apple_library_for_distribution_macro_stub,
    "apple_macos_bundle": _apple_macos_bundle_macro_stub,
    "apple_metal_library": _apple_metal_library_macro_stub,
    "apple_package": _apple_package_macro_stub,
    "apple_test": _apple_test_macro_stub,
    "apple_universal_executable": _apple_universal_executable_macro_stub,
    "apple_watchos_bundle": _apple_watchos_bundle_macro_stub,
    "apple_xcuitest": _apple_xcuitest_macro_stub,
    "configured_alias": _configured_alias_macro_stub,
    "cxx_toolchain": _cxx_toolchain_macro_stub,
    "cxx_toolchain_override": _cxx_toolchain_override_macro_stub,
    "erlang_application": _erlang_application_macro_stub,
    "erlang_tests": _erlang_tests_macro_stub,
    "export_file": _export_file_macro_stub,
    "prebuilt_apple_framework": _prebuilt_apple_framework_macro_stub,
    "prebuilt_apple_xcframework": _prebuilt_apple_xcframework_macro_stub,
    "rust_binary": _rust_binary_macro_stub,
    "rust_library": _rust_library_macro_stub,
    "rust_test": _rust_test_macro_stub,
    "rust_with_workspace": with_rust_workspace,
    "swift_toolchain": _swift_toolchain_macro_stub,
}

__overridden_builtins__ = (
    {
        "read_config": _read_config,
        "read_root_config": _read_root_config,
    }
    if log_buckconfigs
    else {}
)

__shimmed_native__ = __struct_to_dict(__buck2_builtins__)
__shimmed_native__.update(__overridden_builtins__)
__shimmed_native__.update(__rules__)
__shimmed_native__.update(_user_rules)

# Should come after the rules which are macro overridden
__shimmed_native__.update(__extra_rules__)
__shimmed_native__.update({"cxx": _cxx, "python": _python})
__shimmed_native__.update({
    "__internal_autodeps_hacks__": struct(
        rust_link_info = RustLinkInfo,
        rust_sources = RustSources,
    ),
})

native = struct(**__shimmed_native__)
