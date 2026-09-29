# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load(
    "@prelude//platforms/apple:build_mode.bzl",
    "APPLE_BUILD_MODES",
    "CONSTRAINT_PACKAGE",
    "get_build_mode",
)
load(
    "@prelude//platforms/apple:constants.bzl",
    "appletv_platforms",
    "ios_platforms",
    "mac_catalyst_platforms",
    "mac_platforms",
    "watch_platforms",
)

def get_build_mode_constraints_map():
    return {build_mode: ["{}:{}".format(CONSTRAINT_PACKAGE, build_mode)] for build_mode in APPLE_BUILD_MODES}

_MOBILE_PLATFORMS = [
    appletv_platforms.APPLETVOS_ARM64,
    appletv_platforms.APPLETVSIMULATOR_ARM64,
    ios_platforms.IPHONEOS_ARM64,
    ios_platforms.IPHONEOS_ARM64E,
    ios_platforms.IPHONESIMULATOR_ARM64,
    ios_platforms.IPHONESIMULATOR_X86_64,
    watch_platforms.WATCHOS_ARM64,
    watch_platforms.WATCHOS_ARM64_32,
    watch_platforms.WATCHSIMULATOR_ARM64,
    watch_platforms.WATCHSIMULATOR_X86_64,
]

_MAC_PLATFORMS = [
    mac_platforms.MACOS_ARM64,
    mac_platforms.MACOS_X86_64,
    mac_platforms.MACOS_UNIVERSAL,
    mac_catalyst_platforms.MACCATALYST_ARM64,
    mac_catalyst_platforms.MACCATALYST_X86_64,
]

def apple_build_mode_backed_platform(name, platform, build_mode = None):
    build_mode = get_build_mode() if build_mode == None else build_mode
    return _get_generated_name(name, platform, build_mode)

def is_mobile_platform(platform):
    # These build modes are primarily used in mobile code.
    return platform in _MOBILE_PLATFORMS

def is_buck2_mac_platform(platform):
    return platform in _MAC_PLATFORMS

def _get_generated_name(name, platform, build_mode):
    if is_mobile_platform(platform) or is_buck2_mac_platform(platform):
        return "{}-{}".format(name, build_mode)
    else:
        return name
