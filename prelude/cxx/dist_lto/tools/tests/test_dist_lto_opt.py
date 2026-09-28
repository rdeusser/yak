#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

import unittest

from cxx.dist_lto.tools.dist_lto_opt_gnu import _fbcc_prefix_end, _filter_flags


class TestDistLtoOpt(unittest.TestCase):
    def test_filter_flags(self):
        inputs = [
            "-not-opt-flag",
            "-Wl,-plugin-opt,-llvm_flag1",
            "-Wl,-plugin-opt=-llvm_flag1_different_format",
            "-Wl,-plugin-opt,sample-profile=/repo/abc/autofdo.profile",
            "-Wl,--lto-whole-program-visibility",
            "-Wl,-mllvm,-llvm-flag2",
            "-Wl,-plugin-opt,O3",
            "-Wl,-plugin-opt,-function-sections",
            "-Wl,-mllvm,-hot-callsite-threshold=12000",
            "-shared",
        ]
        flags = _filter_flags(inputs)
        self.assertListEqual(
            flags,
            [
                "-O2",
                "-ffunction-sections",
                "-fdata-sections",
                "-mllvm",
                "-llvm_flag1",
                "-mllvm",
                "-llvm_flag1_different_format",
                "-fprofile-sample-use=/repo/abc/autofdo.profile",
                "-mllvm",
                "-llvm-flag2",
                "-O3",
                "-mllvm",
                "-hot-callsite-threshold=12000",
                "-fPIC",
            ],
        )

    def test_fbcc_prefix_end_prebuilt_with_log_fbcc(self):
        """Prebuilt toolchain: --log-fbcc is consumed by fbcc and included in prefix."""
        opt_args = [
            "--",
            "buck-out/fbcc",
            "--cc=third-party/toolchains/build/llvm/19/bin/clang++",
            "--log-fbcc=False",
            "--target=x86_64-redhat-linux-gnu",
            "-nostdinc",
        ]
        self.assertEqual(_fbcc_prefix_end(opt_args), 4)

    def test_fbcc_prefix_end_buckified_no_log_fbcc(self):
        """Buckified toolchain: --target is NOT consumed by fbcc and must be
        excluded from prefix to avoid breaking -cc1 mode."""
        opt_args = [
            "--",
            "buck-out/fbcc",
            "--cc=buck-out/v2/art/root/third-party/llvm/19/__build/bin/clang++__/out/clang++",
            "--target=x86_64-redhat-linux-gnu",
            "-nostdinc",
            "-nostdinc++",
        ]
        self.assertEqual(_fbcc_prefix_end(opt_args), 3)

    def test_fbcc_prefix_end_minimal(self):
        """Minimal case: only --cc= present."""
        opt_args = [
            "--",
            "buck-out/fbcc",
            "--cc=some/clang++",
        ]
        self.assertEqual(_fbcc_prefix_end(opt_args), 3)

    def test_fbcc_prefix_end_with_fbcc_debug_info(self):
        """--fbcc-create-external-debug-info is consumed by fbcc."""
        opt_args = [
            "--",
            "buck-out/fbcc",
            "--cc=some/clang++",
            "--fbcc-create-external-debug-info=/tmp/foo.dwo",
            "--target=x86_64-redhat-linux-gnu",
        ]
        self.assertEqual(_fbcc_prefix_end(opt_args), 4)

    def test_fbcc_prefix_end_multiple_consumed_flags(self):
        """Multiple fbcc-consumed flags in a row."""
        opt_args = [
            "--",
            "buck-out/fbcc",
            "--cc=some/clang++",
            "--log-fbcc=True",
            "--fbcc-create-external-debug-info=/tmp/foo.dwo",
            "--target=x86_64-redhat-linux-gnu",
        ]
        self.assertEqual(_fbcc_prefix_end(opt_args), 5)

    def test_filter_flags_lto_case(self):
        inputs = [
            "--target=x86_64-redhat-linux-gnu",
            "-nostdinc",
            "-resource-dir",
            "-idirafter",
            "-idirafter",
            "third-party/toolchains/build/glibc/include",
            "-idirafter",
            "third-party/toolchains/build/kernel-headers/include",
            "-Bthird-party/toolchains/build/binutils/x86_64-pc-linux-gnu/bin",
            "--cflag=--target=x86_64-redhat-linux-gnu",
            "-Bthird-party/toolchains/build/glibc/lib",
            "-Bthird-party/toolchains/tools/gcc/lib/gcc/x86_64-redhat-linux-gnu/trunk",
            "-Lthird-party/toolchains/build/libgcc/lib/gcc/x86_64-pc-linux-gnu/trunk",
            "-Wl,-nostdlib",
            "-Wl,--dynamic-linker,/usr/local/toolchains/lib/ld.so",
            "-Wl,--disable-new-dtags",
            "-Bthird-party/toolchains/build/binutils/x86_64-pc-linux-gnu/bin",
            "-Wl,--no-mmap-output-file",
            "-nodefaultlibs",
            "--target=x86_64-redhat-linux-gnu",
            "-Lthird-party/toolchains/build/glibc/lib",
            "-Lthird-party/toolchains/build/libgcc/lib",
            "-Wl,-z,notext",
            "-Wl,-z,relro",
            "-Wl,--gc-sections",
            "-fuse-ld=lld",
            "-Wl,--discard-section=.nv_fatbin",
            "-Wl,--discard-section=.nvFatBinSegment",
            "-Wl,--discard-section=.rela.debug_info",
            "-Wl,--discard-section=.rela.debug_ranges",
            "-Wl,--discard-section=.rela.debug_loc",
            "-Wl,--discard-section=.rela.debug_line",
            "-Wl,--discard-section=.rela.debug_aranges",
            "-Wl,--discard-section=.rela.debug_types",
            "-Wl,-O1",
            "-Wl,--build-id=sha1",
            "-Wl,-mllvm,-hot-callsite-threshold=12000",
            "-Wl,--lto-whole-program-visibility",
            "-fwhole-program-vtables",
            "-Wl,--no-discard-section=.nv_fatbin",
            "-Wl,--no-discard-section=.nvFatBinSegment",
            "tools/build/move_gpu_sections_implicit_linker_script.txt",
            "-fuse-ld=lld",
            "--build-info=full",
            "--build-info-build-mode=opt-lto",
            "--build-info-build-tool=buck2",
            "--build-info-compiler=clang",
            "--build-info-fdo-profile=",
            "--build-info-platform=linux",
            "--build-info-rule=root:scripts/minimal:minimal",
            "--build-info-rule-type=cpp_binary",
            "-flto=thin",
            "-Wl,-plugin-opt,-function-sections",
            "-Wl,-plugin-opt,-profile-guided-section-prefix=false",
            "-Wl,-plugin-opt,-generate-type-units",
            "-Wl,-plugin-opt,-enable-lto-ir-verification=false",
            "-Xlinker",
            "--export-dynamic-symbol=mallocx",
            "-Xlinker",
            "--export-dynamic-symbol=rallocx",
            "-Xlinker",
            "--export-dynamic-symbol=xallocx",
            "-Xlinker",
            "--export-dynamic-symbol=sallocx",
            "-Xlinker",
            "--export-dynamic-symbol=dallocx",
            "-Xlinker",
            "--export-dynamic-symbol=sdallocx",
            "-Xlinker",
            "--export-dynamic-symbol=nallocx",
            "-Xlinker",
            "--export-dynamic-symbol=mallctl",
            "-Xlinker",
            "--export-dynamic-symbol=mallctlnametomib",
            "-Xlinker",
            "--export-dynamic-symbol=mallctlbymib",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_stats_print",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_usable_size",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_message",
        ]
        flags = _filter_flags(inputs)
        self.assertIsNotNone(flags)
        self.assertListEqual(
            flags,
            [
                "-O2",
                "-ffunction-sections",
                "-fdata-sections",
                "-fuse-ld=lld",
                "-mllvm",
                "-hot-callsite-threshold=12000",
                "-fwhole-program-vtables",
                "-fuse-ld=lld",
                "-mllvm",
                "-profile-guided-section-prefix=false",
                "-mllvm",
                "-generate-type-units",
                "-mllvm",
                "-enable-lto-ir-verification=false",
            ],
        )

    def test_filter_flags_fdo_case(self):
        inputs = [
            "--cflag=--target=x86_64-redhat-linux-gnu",
            "-Bthird-party/toolchains/build/glibc/lib",
            "-Bthird-party/toolchains/tools/gcc/lib/gcc/x86_64-redhat-linux-gnu/trunk",
            "-Lthird-party/toolchains/build/libgcc/lib/gcc/x86_64-pc-linux-gnu/trunk",
            "-Wl,-nostdlib",
            "-Wl,--dynamic-linker,/usr/local/toolchains/lib/ld.so",
            "-Wl,--disable-new-dtags",
            "-Bthird-party/toolchains/build/binutils/x86_64-pc-linux-gnu/bin",
            "-Wl,--no-mmap-output-file",
            "-nodefaultlibs",
            "--target=x86_64-redhat-linux-gnu",
            "-Lthird-party/toolchains/build/glibc/lib",
            "-Lthird-party/toolchains/build/libgcc/lib",
            "-Wl,-z,notext",
            "-Wl,-z,relro",
            "-Wl,--gc-sections",
            "-fuse-ld=lld",
            "-Wl,--discard-section=.nv_fatbin",
            "-Wl,--discard-section=.nvFatBinSegment",
            "-Wl,--discard-section=.rela.debug_info",
            "-Wl,--discard-section=.rela.debug_ranges",
            "-Wl,--discard-section=.rela.debug_loc",
            "-Wl,--discard-section=.rela.debug_line",
            "-Wl,--discard-section=.rela.debug_aranges",
            "-Wl,--discard-section=.rela.debug_types",
            "-Wl,-O1",
            "-Wl,--build-id=sha1",
            "-Xlinker",
            "-znow",
            "-Xlinker",
            "--emit-relocs",
            "--build-info=full",
            "--build-info-build-mode=opt-clang-thinlto",
            "--build-info-build-tool=buck2",
            "--build-info-compiler=clang",
            "--build-info-fdo-profile=root//fdo/autofdo/default_profile:autofdo",
            "--build-info-platform=linux",
            "--build-info-rule=root:server:index_server",
            "--build-info-rule-type=cpp_binary",
            "-flto=thin",
            "-Wl,-plugin-opt,sample-profile=buck-out/v2/gen/root/40fc99293b37c503/fdo/autofdo/default_profile/__autofdo__/out/profile",
            "-Wl,-plugin-opt,-function-sections",
            "-Wl,-plugin-opt,-profile-guided-section-prefix=false",
            "-Wl,-plugin-opt,-generate-type-units",
            "-Wl,-plugin-opt,-enable-lto-ir-verification=false",
            "-Xlinker",
            "--push-state",
            "-Xlinker",
            "--no-as-needed",
            "-Xlinker",
            "--pop-state",
            "-Wl,--undefined,Global",
            "-Wl,--undefined,Local",
            "-Wl,--undefined,MockConnection",
            "-Wl,--undefined,Global",
            "-Wl,--undefined,Local",
            "-Wl,--undefined,MockConnection",
            "-Xlinker",
            "--start-group",
            "third-party/toolchains/build/IntelComposerXE/mkl/lib/intel64/libmkl_intel_lp64.a",
            "third-party/toolchains/build/IntelComposerXE/mkl/lib/intel64/libmkl_core.a",
            "third-party/toolchains/build/IntelComposerXE/mkl/lib/intel64/libmkl_intel_thread.a",
            "-Xlinker",
            "--end-group",
            "-lpthread",
            "-Xlinker",
            "--export-dynamic-symbol=mallocx",
            "-Xlinker",
            "--export-dynamic-symbol=rallocx",
            "-Xlinker",
            "--export-dynamic-symbol=xallocx",
            "-Xlinker",
            "--export-dynamic-symbol=sallocx",
            "-Xlinker",
            "--export-dynamic-symbol=dallocx",
            "-Xlinker",
            "--export-dynamic-symbol=sdallocx",
            "-Xlinker",
            "--export-dynamic-symbol=nallocx",
            "-Xlinker",
            "--export-dynamic-symbol=mallctl",
            "-Xlinker",
            "--export-dynamic-symbol=mallctlnametomib",
            "-Xlinker",
            "--export-dynamic-symbol=mallctlbymib",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_stats_print",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_usable_size",
            "-Xlinker",
            "--export-dynamic-symbol=malloc_message",
        ]
        flags = _filter_flags(inputs)
        self.assertIsNotNone(flags)
        self.assertListEqual(
            flags,
            [
                "-O2",
                "-ffunction-sections",
                "-fdata-sections",
                "-fuse-ld=lld",
                "-fprofile-sample-use=buck-out/v2/gen/root/40fc99293b37c503/fdo/autofdo/default_profile/__autofdo__/out/profile",
                "-mllvm",
                "-profile-guided-section-prefix=false",
                "-mllvm",
                "-generate-type-units",
                "-mllvm",
                "-enable-lto-ir-verification=false",
            ],
        )
