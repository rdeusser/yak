# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//android:android_providers.bzl", "AndroidApkExopackageInfo", "AndroidApkInfo", "AndroidInstrumentationApkInfo")
load("@prelude//android:android_toolchain.bzl", "AndroidToolchainInfo")
load("@prelude//java:class_to_srcs.bzl", "JavaClassToSourceMapInfo")
load("@prelude//java:java_providers.bzl", "JavaPackagingInfo", "get_all_java_packaging_deps_tset")
load("@prelude//java:java_toolchain.bzl", "JavaToolchainInfo")
load("@prelude//java/utils:java_more_utils.bzl", "get_path_separator_for_exec_os")
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
    "create_shlib_symlink_tree",
    "merge_shared_libraries",
    "traverse_shared_library_info",
)
load("@prelude//test:inject_test_run_info.bzl", "inject_test_run_info")
load("@prelude//utils:argfile.bzl", "at_argfile")
load("@prelude//utils:expect.bzl", "expect")

def android_instrumentation_test_impl(ctx: AnalysisContext):
    android_toolchain = ctx.attrs._android_toolchain[AndroidToolchainInfo]

    cmd = [ctx.attrs._java_test_toolchain[JavaToolchainInfo].java_for_tests]

    classpath = android_toolchain.instrumentation_test_runner_classpath

    classpath_args = cmd_args()
    classpath_args.add("-classpath")
    env = ctx.attrs.env or {}
    extra_classpath = []
    if ctx.attrs.instrumentation_test_listener != None:
        extra_classpath.extend([
            get_all_java_packaging_deps_tset(ctx, java_packaging_infos = [ctx.attrs.instrumentation_test_listener[JavaPackagingInfo]]).project_as_args(
                "full_jar_args", ordering = "bfs"
            ),
        ])

        shared_library_info = merge_shared_libraries(
            ctx.actions,
            deps = [ctx.attrs.instrumentation_test_listener[SharedLibraryInfo]],
        )

        cxx_library_symlink_tree = create_shlib_symlink_tree(
            actions = ctx.actions,
            out = "cxx_library_symlink_tree",
            shared_libs = traverse_shared_library_info(shared_library_info, transformation_provider = None),
        )

        env["YAK_LD_SYMLINK_TREE"] = cxx_library_symlink_tree
    classpath_args.add(cmd_args(extra_classpath + classpath, delimiter = get_path_separator_for_exec_os(ctx)))
    cmd.append(at_argfile(actions = ctx.actions, name = "classpath_args_file", args = classpath_args))

    cmd.append(android_toolchain.instrumentation_test_runner_main_class)

    apk_info = ctx.attrs.apk.get(AndroidApkInfo)
    expect(apk_info != None, "Provided APK must have AndroidApkInfo!")

    instrumentation_apk_info = ctx.attrs.apk.get(AndroidInstrumentationApkInfo)
    # A self-instrumenting apk already bundles the app-under-test's contents, and its manifest's
    # targetPackage is the self apk itself rather than the un-merged app-under-test apk's package,
    # so there is no separate app-under-test to install.
    if instrumentation_apk_info != None and not instrumentation_apk_info.is_self_instrumenting:
        cmd.extend(["--apk-under-test-path", instrumentation_apk_info.apk_under_test])
    if ctx.attrs.is_self_instrumenting:
        cmd.extend(["--is-self-instrumenting"])
    extra_instrumentation_args = ctx.attrs.extra_instrumentation_args
    if extra_instrumentation_args:
        for arg_name, arg_value in extra_instrumentation_args.items():
            cmd.extend(
                [
                    "--extra-instrumentation-argument",
                    cmd_args([arg_name, arg_value], delimiter = "="),
                ],
            )

    target_package_file = ctx.actions.declare_output("target_package_file", has_content_based_path = False)
    package_file = ctx.actions.declare_output("package_file", has_content_based_path = False)
    test_runner_file = ctx.actions.declare_output("test_runner_file", has_content_based_path = False)
    manifest_utils_cmd = cmd_args(ctx.attrs._android_toolchain[AndroidToolchainInfo].manifest_utils[RunInfo])
    manifest_utils_cmd.add([
        "--manifest-path",
        apk_info.manifest,
        "--package-output",
        package_file.as_output(),
        "--target-package-output",
        target_package_file.as_output(),
        "--instrumentation-test-runner-output",
        test_runner_file.as_output(),
    ])
    ctx.actions.run(manifest_utils_cmd, category = "get_manifest_info")
    cmd.extend(
        [
            "--test-package-name",
            cmd_args(package_file, format = "@{}"),
            "--target-package-name",
            cmd_args(target_package_file, format = "@{}"),
            "--test-runner",
            cmd_args(test_runner_file, format = "@{}"),
        ],
    )

    if ctx.attrs.instrumentation_test_listener_class != None:
        cmd.extend(["--extra-instrumentation-test-listener", ctx.attrs.instrumentation_test_listener_class])

    if ctx.attrs.clear_package_data:
        cmd.append("--clear-package-data")

    if ctx.attrs.disable_animations:
        cmd.append("--disable-animations")

    if ctx.attrs.collect_tombstones:
        cmd.append("--collect-tombstones")
    if ctx.attrs.record_video:
        cmd.append("--record-video")
    if android_toolchain.collect_perfetto:
        cmd.append("--collect-perfetto")
    if ctx.attrs.log_extractors:
        for arg_name, arg_value in ctx.attrs.log_extractors.items():
            cmd.extend(
                [
                    "--log-extractor",
                    cmd_args([arg_name, arg_value], delimiter = "="),
                ],
            )

    cmd.extend(
        [
            "--adb-executable-path",
            "required_but_unused",
            "--instrumentation-apk-path",
            apk_info.apk,
        ],
    )

    # Exopackage secondary dexes live out-of-band, not in base.apk. Pass the build's secondary-dex
    # dir so the runner pushes them to /data/local/tmp/exopackage/<pkg>/secondary-dex before
    # Application init; otherwise classes in those dexes (e.g. com.example.R$style) hit NoClassDefFoundError on RE.
    apk_exopackage_info = ctx.attrs.apk.get(AndroidApkExopackageInfo)
    if apk_exopackage_info != None:
        cmd.extend([
            "--exopackage-secondary-dex-local-dir",
            apk_exopackage_info.secondary_dex_directory,
        ])

    test_info = ExternalRunnerTestInfo(
        type = "android_instrumentation",
        command = cmd,
        env = env,
        labels = ctx.attrs.labels,
        contacts = ctx.attrs.contacts,
        run_from_project_root = True,
        use_project_relative_paths = True,
        executor_overrides = _compute_executor_overrides(ctx, android_toolchain.instrumentation_test_can_run_locally),
        local_resources = {
            "android_emulator": None if ctx.attrs._android_emulators == None else ctx.attrs._android_emulators.label,
        },
        required_local_resources = [RequiredTestLocalResource("android_emulator", listing = True, execution = True)],
    )

    classmap_source_info = [ctx.attrs.apk[JavaClassToSourceMapInfo]] if JavaClassToSourceMapInfo in ctx.attrs.apk else []

    test_info, run_info = inject_test_run_info(ctx, test_info)

    # We append additional args so that "yak run" will work with sane defaults
    run_info.args.add(cmd_args(["--auto-run-on-connected-device", "--output", ".", "--adb-executable-path", "adb"]))
    return [
        test_info,
        run_info,
        DefaultInfo(),
    ] + classmap_source_info

def _compute_executor_overrides(ctx: AnalysisContext, instrumentation_test_can_run_locally: bool) -> dict[str, CommandExecutorConfig]:
    # A test executor requests one of these overrides by name for a test stage.
    re_caps = ctx.attrs.re_caps or {}
    re_use_case = ctx.attrs.re_use_case or {}
    expect(
        sorted(re_caps.keys()) == sorted(re_use_case.keys()),
        "`re_caps` and `re_use_case` must name the same executor overrides, got [{}] and [{}]",
        ", ".join(re_caps.keys()),
        ", ".join(re_use_case.keys()),
    )
    return {
        name: CommandExecutorConfig(
            local_enabled = instrumentation_test_can_run_locally,
            remote_enabled = True,
            remote_execution_properties = capabilities,
            remote_execution_use_case = re_use_case[name],
        )
        for name, capabilities in re_caps.items()
    }
