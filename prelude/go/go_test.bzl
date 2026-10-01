# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load(
    "@prelude//linking:link_info.bzl",
    "LinkStyle",
)
load("@prelude//test:inject_test_run_info.bzl", "inject_test_run_info")
load(
    "@prelude//tests:re_utils.bzl",
    "get_re_executors_from_props",
)
load(
    "@prelude//utils:utils.bzl",
    "from_named_set",
    "map_val",
    "value_or",
)
load(":cgo_builder.bzl", "get_cgo_build_context")
load(":compile.bzl", "GoPkgCompileInfo", "GoTestInfo")
load(":coverage.bzl", "GoCoverageMode")
load(":link.bzl", "GoBuildMode", "get_inherited_link_pkgs", "link")
load(":package_builder.bzl", "GoBuildConfig", "GoSourceInputs", "declare_package_build")
load(":packages.bzl", "go_attr_pkg_name")
load(":toolchain.bzl", "GoToolchainInfo", "evaluate_cgo_enabled")

def _gen_test_main(
    ctx: AnalysisContext,
    pkg_import_path: str,
    coverage_mode: [GoCoverageMode, None],
    cover_packages: list[str],  # packages those are included for coverage
    test_go_files_argsfile: Artifact,
    xtest_go_files_argsfile: Artifact,
) -> Artifact:
    """
    Generate a `main.go` which calls tests from the given sources.
    """
    output = ctx.actions.declare_output("main.go", has_content_based_path = True)
    cmd = []
    cmd.append(ctx.attrs._testmaingen[RunInfo])

    # if ctx.attrs.coverage_mode:
    cmd.append(cmd_args(output.as_output(), format = "--output={}"))
    cmd.append(cmd_args(pkg_import_path, format = "--import-path={}"))

    # The test runs in the directory of its resources, which sits next to the binary that `link`
    # writes to the target's name.
    working_directory = [_resources_dir_name(ctx).rsplit("/", 1)[-1]]
    if ctx.attrs.working_directory:
        working_directory.append(ctx.attrs.working_directory)
    cmd.append(cmd_args("/".join(working_directory), format = "--working-directory={}"))
    if coverage_mode != None:
        cmd.extend(["--cover-mode", coverage_mode.value])
    if cover_packages:
        cover_pkgs_argsfile = ctx.actions.declare_output("cover_pkgs_argsfile", has_content_based_path = True)
        ctx.actions.write(cover_pkgs_argsfile, [["--cover-pkgs", pkg] for pkg in cover_packages])
        cmd.append(cmd_args(cover_pkgs_argsfile, format = "@{}"))
    # The flags of the external test files come before the internal test files, which are
    # positional arguments.
    cmd.append(cmd_args(xtest_go_files_argsfile, format = "@{}"))
    cmd.append(cmd_args(test_go_files_argsfile, format = "@{}"))
    ctx.actions.run(cmd_args(cmd), category = "go_test_main_gen", allow_cache_upload = ctx.attrs._go_toolchain[GoToolchainInfo].allow_cache_upload)
    return output

def _resources_dir_name(ctx: AnalysisContext) -> str:
    return ctx.label.name + "-resources"

def is_subpackage_of(other_pkg_import_path: str, pkg_import_path: str) -> bool:
    return pkg_import_path == other_pkg_import_path or other_pkg_import_path.startswith(pkg_import_path + "/")

def go_test_impl(ctx: AnalysisContext) -> list[Provider]:
    pkg_import_path = go_attr_pkg_name(ctx)
    cgo_enabled = evaluate_cgo_enabled(ctx.attrs._cgo_enabled)

    deps = ctx.attrs.deps
    srcs = ctx.attrs.srcs
    coverage_enabled = ctx.attrs.coverage_enabled

    # Copy the srcs, deps and pkg_import_path from the target library when set. The
    # library code gets compiled together with the tests.
    if ctx.attrs.target_under_test:
        lib = ctx.attrs.target_under_test[GoTestInfo]
        srcs += lib.srcs
        deps += lib.deps

        # TODO: should we assert that pkg_import_path != None here?
        pkg_import_path = lib.pkg_import_path
        coverage_enabled = coverage_enabled or lib.coverage_enabled

    # If coverage is enabled for this test, we need to preprocess the sources
    # with the Go cover tool.
    coverage_mode = GoCoverageMode(ctx.attrs._coverage_mode) if ctx.attrs._coverage_mode else None
    cgo_build_context = get_cgo_build_context(ctx)
    pkgs = {}

    # Compile all tests into a package.
    tests, tests_pkg_info, test_go_files_argsfile = declare_package_build(
        ctx = ctx,
        pkg_import_path = pkg_import_path,
        main = False,
        sources = GoSourceInputs(
            srcs = srcs,
            embed_srcs = from_named_set(ctx.attrs.embed_srcs),
            package_root = ctx.attrs.package_root,
        ),
        cgo_build_context = cgo_build_context,
        config = GoBuildConfig(
            compiler_flags = ctx.attrs.compiler_flags,
            build_tags = ctx.attrs._build_tags,
            coverage_enabled = coverage_enabled,
            coverage_mode = coverage_mode,
            with_tests = True,
            cgo_enabled = cgo_enabled,
        ),
        pkgs = pkgs,
        deps = deps,
    )

    # A package without test files of its own is linked as `target_under_test` builds it, so
    # that the dependencies of its external tests that import it link the same package.
    under_test = tests
    if ctx.attrs.external_tests_only:
        if not ctx.attrs.target_under_test:
            fail("`external_tests_only` needs `target_under_test`")
        under_test = ctx.attrs.target_under_test[GoPkgCompileInfo].pkgs[pkg_import_path]

    cover_packages = []

    # Cover the test package itself
    if under_test.coverage_instrumented:
        cover_packages.append(pkg_import_path)

    # Get all packages that are linked to the test (i.e. the entire dependency tree)
    for import_path, pkg in get_inherited_link_pkgs(deps).items():
        if pkg.coverage_instrumented:
            # Cover dependencies with coverage instrumented
            cover_packages.append(import_path)

    pkgs[pkg_import_path] = under_test

    # Compile the external tests (`package <name>_test`) into a package of their own, which
    # imports the package above with its tests, as `go test` does.
    xtests, _, xtest_go_files_argsfile = declare_package_build(
        ctx = ctx,
        pkg_import_path = pkg_import_path + "_test",
        main = False,
        sources = GoSourceInputs(
            srcs = srcs,
            embed_srcs = from_named_set(ctx.attrs.embed_srcs),
            package_root = ctx.attrs.package_root,
        ),
        cgo_build_context = cgo_build_context,
        config = GoBuildConfig(
            compiler_flags = ctx.attrs.compiler_flags,
            build_tags = ctx.attrs._build_tags,
            cgo_enabled = cgo_enabled,
            x_tests = True,
        ),
        pkgs = {pkg_import_path: under_test},
        deps = deps,
        cgo_gen_dir_name = "cgo_gen_xtest",
    )
    pkgs[pkg_import_path + "_test"] = xtests

    # Generate a 'main.go' file (test runner) which runs the actual tests from the package above.
    # Build the it as a separate package (<foo>.test) - which imports and invokes the test package.
    gen_main = _gen_test_main(ctx, pkg_import_path, coverage_mode, cover_packages, test_go_files_argsfile, xtest_go_files_argsfile)
    main, _, _ = declare_package_build(
        ctx = ctx,
        pkg_import_path = pkg_import_path + ".test",
        main = True,
        sources = GoSourceInputs(
            srcs = [gen_main],
            package_root = "",
        ),
        cgo_build_context = None,
        config = GoBuildConfig(
            cgo_enabled = cgo_enabled,
        ),
        pkgs = pkgs,
        cgo_gen_dir_name = "cgo_gen_test_main",
    )

    # Link the above into a Go binary.
    (bin, runtime_files, external_debug_info) = link(
        ctx = ctx,
        main = main,
        cgo_enabled = cgo_enabled,
        pkgs = pkgs,
        deps = deps,
        link_style = value_or(map_val(LinkStyle, ctx.attrs.link_style), LinkStyle("static")),
        build_mode = GoBuildMode(value_or(ctx.attrs.build_mode, "exe")),
        linker_flags = ctx.attrs.linker_flags,
        external_linker_flags = ctx.attrs.external_linker_flags,
    )

    # Copy the resources into one directory next to the binary, at their paths in the package. The
    # directory is one output, so a resource that the target no longer lists leaves it.
    resources_dir = ctx.actions.copied_dir(
        _resources_dir_name(ctx),
        {resource.short_path: resource for resource in ctx.attrs.resources},
        has_content_based_path = False,
    )

    run_cmd = cmd_args(bin, hidden = [runtime_files, external_debug_info, resources_dir])

    # Setup RE executors based on the `remote_execution` param.
    re_executors = get_re_executors_from_props(ctx)

    return inject_test_run_info(
        ctx,
        ExternalRunnerTestInfo(
            type = "go",
            command = [run_cmd],
            env = ctx.attrs.env,
            labels = ctx.attrs.labels,
            contacts = ctx.attrs.contacts,
            default_executor = re_executors.default_executor,
            executor_overrides = re_executors.executor_overrides,
            run_from_project_root = True,
            use_project_relative_paths = re_executors.use_project_relative_paths,
            supports_test_execution_caching = ctx.attrs.supports_test_execution_caching,
        ),
    ) + [
        DefaultInfo(
            default_output = bin,
            other_outputs = [gen_main, resources_dir] + runtime_files + external_debug_info,
        ),
        tests_pkg_info,
    ]
