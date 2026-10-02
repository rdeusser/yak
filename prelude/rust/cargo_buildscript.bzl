# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Cargo build script runner for generated third-party Rust targets.
#
#     # optional (this matches the default rule name):
#     buildscript_genrule = "buildscript_run"
#

load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo", "LinkerType")
load(
    "@prelude//cxx:preprocessor.bzl",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
)
load("@prelude//cxx:target_sdk_version.bzl", "get_target_triple")
load("@prelude//decls:common.bzl", "yak")
load("@prelude//decls:toolchains_common.bzl", "toolchains_common")
load("@prelude//linking:link_groups.bzl", "EMPTY_LINK_GROUP_LIB_INFO")
load(
    "@prelude//linking:link_info.bzl",
    "LibOutputStyle",
    "LinkInfo",
    "LinkInfos",
    "LinkInfosTSet",
    "LinkStrategy",
    "MergedLinkInfo",
    "create_merged_link_info",
)
load("@prelude//linking:linkable_graph.bzl", "create_linkable_graph")
load("@prelude//linking:shared_libraries.bzl", "EMPTY_SHARED_LIBRARY_INFO")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup", "ScriptLanguage")
load("@prelude//rust:rust_toolchain.bzl", "RustToolchainInfo")
load("@prelude//rust:targets.bzl", "targets")
load("@prelude//rust/buildscript:buildscript_platform.bzl", "transition_alias")
load("@prelude//rust/tools:attrs.bzl", "RustInternalToolsInfo")
load("@prelude//utils:cmd_script.bzl", "cmd_script")
load("@prelude//utils:selects.bzl", "selects")
load(":build.bzl", "dependency_args")
load(":build_params.bzl", "MetadataKind")
load(
    ":cargo_package.bzl",
    "apply_platform_attrs",
    "get_cargo_platform_names",
    "get_cargo_platforms",
)
load(":dep_context.bzl", "DepCollectionContext")
load(
    ":link_info.bzl",
    "BuildScriptSharedLibDirs",
    "BuildScriptSharedLibsInfo",
    "DEFAULT_STATIC_LINK_STRATEGY",
    "RustProcMacroPlugin",
    "gather_explicit_sysroot_deps",
    "resolve_rust_deps_inner",
)
load(":rust_toolchain.bzl", "PanicRuntime")
load(":sources.bzl", "package_srcs_files")

# The metadata that a build script of a package with `links` prints, which Cargo passes to the
# build scripts of the packages that depend on it as `DEP_<links>_<key>`.
BuildScriptMetadataInfo = provider(fields = {
    # The `DEP_<links>_<key>=<value>` lines, with placeholders for paths in the script's
    # `OUT_DIR` and manifest directory.
    "metadata": provider_field(Artifact),
    "out_dir": provider_field(Artifact),
    "manifest_dir": provider_field(Artifact),
})

def _make_rustc_shim(ctx: AnalysisContext, cwd: Artifact) -> cmd_args:
    # Build scripts expect to receive a `rustc` which "just works." However,
    # our rustc sometimes has no sysroot available, so we need to make a shim
    # which supplies the sysroot deps if necessary
    toolchain_info = ctx.attrs._rust_toolchain[RustToolchainInfo]
    explicit_sysroot_deps = toolchain_info.explicit_sysroot_deps
    if explicit_sysroot_deps:
        dep_ctx = DepCollectionContext(
            advanced_unstable_linking = False,
            include_doc_deps = False,
            is_proc_macro = False,
            explicit_sysroot_deps = explicit_sysroot_deps,
            panic_runtime = PanicRuntime("unwind"),  # not actually used
        )
        deps = gather_explicit_sysroot_deps(dep_ctx)
        deps = resolve_rust_deps_inner(ctx, deps)
        dep_args, dep_argsfiles, _ = dependency_args(
            ctx = ctx,
            internal_tools_info = ctx.attrs._rust_internal_tools_toolchain[RustInternalToolsInfo],
            transitive_dependency_dirs = set(),
            toolchain_info = toolchain_info,
            deps = deps,
            subdir = "any",
            dep_link_strategy = DEFAULT_STATIC_LINK_STRATEGY,
            dep_metadata_kind = MetadataKind("full"),
            is_rustdoc_test = False,
            cwd = cwd,
        )

        null_path = "nul" if ctx.attrs._exec_os_type[OsLookup].os == Os("windows") else "/dev/null"
        dep_args = cmd_args("--sysroot=" + null_path, dep_args, relative_to = cwd)
        dep_file, _ = ctx.actions.write("rustc_dep_file", dep_args, allow_args = True, has_content_based_path = False)
        sysroot_args = cmd_args(
            cmd_args("@", dep_file, delimiter = "", hidden = dep_args),
            # add dep_argsfiles as a separate argument because rustc does NOT support nested @argsfiles
            dep_argsfiles,
        )
    else:
        sysroot_args = cmd_args()

    shim = cmd_script(
        actions = ctx.actions,
        name = "__rustc_shim",
        cmd = cmd_args(toolchain_info.compiler, sysroot_args, relative_to = cwd),
        language = ctx.attrs._exec_os_type[OsLookup].script,
    )

    return cmd_args(shim, relative_to = cwd)

def _make_cc_shim(ctx: AnalysisContext, name: str, cmd: cmd_args) -> cmd_args:
    """
    Wrap cmd, which can contain relative paths to executables and various
    resources, in a script that can be invoked from any directory.

    Different crates' build scripts run $CC from inside of $OUT_DIR, or from
    /tmp, not necessarily only from the directory that yak gives to the build
    script execution. Also they pass arguments to $CC which are relative to the
    directory they chose to run it in.

    For a cmd like this:

        yak-out/v2/art/root/tools/build/__cc_wrapper__/0cdd64957fa390c4/cc_wrapper \
        -resource-dir \
        third-party/toolchains/build/llvm/21/lib/clang/stable \
        -Bthird-party/toolchains/build/binutils/x86_64-pc-linux-gnu/bin

    we use `absolute_prefix` to insert a recognizable marker `${..}/` in front
    of every argument that is a path. The markers are later substituted with an
    appropriate value after learning the buildscript-selected working directory.

        python3 \
        prelude/rust/tools/from_any_dir.py \
        --cwd=/re_cwd/yak-out/v2/art/root/eef091ffd45259ca/third-party/rust/vendor/gmp-mpfr-sys/__1-build-script-run__/cwd \
        ${..}/yak-out/v2/art/root/tools/build/__cc_wrapper__/0cdd64957fa390c4/cc_wrapper \
        -resource-dir \
        ${..}/third-party/toolchains/build/llvm/21/lib/clang/stable \
        -B${..}/third-party/toolchains/build/binutils/x86_64-pc-linux-gnu/bin

    There are 4 categories of paths which are relative to different locations.

    1. Paths in the RunInfo of the from_any_dir tool, including the path of that
       Python program and paths in the toolchain's Python interpreter and global
       arguments to the Python interpreter. These are relative to the repo root
       and we do not prepend the marker. The wrapper script must change
       directory to the repo root before evaluating these paths.

    2. The buildscript-selected working directory. This is captured as an
       absolute path by the wrapper script and passed to from_any_dir's `--cwd`
       flag.

    3. Paths in `cmd`. We mark these for from_any_dir to rewrite relative to the
       buildscript-selected working directory. From_any_dir must change to that
       directory before evaluating these paths.

    4. Paths in the arguments passed by the build script to $CC. These are
       relative to the buildscript-selected working directory and we must have
       changed back to that directory before evaluating these paths.
    """

    internal_tools_info = ctx.attrs._rust_internal_tools_toolchain[RustInternalToolsInfo]

    language = ctx.attrs._exec_os_type[OsLookup].script
    if language == ScriptLanguage("sh"):
        script = ctx.actions.declare_output("{}.sh".format(name), has_content_based_path = True)
        wrapper, _ = ctx.actions.write(
            script,
            [
                "#!/usr/bin/env bash",
                # Capture buildscript-selected working directory.
                "cc_original_dir=$(pwd)",
                # Change directory to the script's location in yak-out, then up
                # to the repo root.
                'cd -- "$(dirname -- "$(realpath "${BASH_SOURCE[0]}")")"',
                cmd_args(ctx.label.project_root, relative_to = (script, 1), format = "cd {}"),
                # Run from_any_dir.py.
                cmd_args(
                    cmd_args(
                        cmd_args(internal_tools_info.from_any_dir, quote = "shell"),
                        '--cwd="${cc_original_dir}"',
                        cmd_args(cmd, absolute_prefix = "${..}/", quote = "shell"),
                        delimiter = " \\\n",
                    ),
                    # For linker, prepend every argument with `-Wl,`. Without this,
                    # when using Clang for the linker, arguments are intercepted
                    # by Clang instead of making it to the actual linker.
                    # >> clang++: error: unknown argument: '--as-needed'
                    format = '{} "${@/#/-Wl,}"\n' if name == "__ld_shim" else '{} "$@"\n',
                ),
            ],
            is_executable = True,
            allow_args = True,
        )
    elif language == ScriptLanguage("bat"):
        script = ctx.actions.declare_output("{}.bat".format(name), has_content_based_path = True)
        wrapper, _ = ctx.actions.write(
            script,
            [
                "@echo off",
                [
                    # Prepend every argument with `-Wl,`.
                    "setlocal enabledelayedexpansion",
                    'set "wl_args="',
                    "for %%a in (%*) do (",
                    '    set "wl_args=!wl_args! -Wl,%%~a"',
                    ")",
                ]
                if name == "__ld_shim"
                else [],
                "set cc_original_dir=%CD%",
                'cd /d "%~dp0"',
                cmd_args(ctx.label.project_root, relative_to = (script, 1), format = "cd {}"),
                cmd_args(
                    cmd_args(
                        cmd_args(internal_tools_info.from_any_dir, quote = "shell"),
                        '--cwd="%cc_original_dir%"',
                        cmd_args(cmd, absolute_prefix = "${..}\\", quote = "shell"),
                        delimiter = " ^\n  ",
                    ),
                    format = "{} !args!\n" if name == "__ld_shim" else "{} %*\n",
                ),
            ],
            is_executable = True,
            allow_args = True,
        )
    else:
        fail(language)

    return cmd_args(wrapper, hidden = [internal_tools_info.from_any_dir, cmd])

def _cargo_buildscript_impl(ctx: AnalysisContext) -> list[Provider]:
    cxx_toolchain_info = ctx.attrs._cxx_toolchain[CxxToolchainInfo]
    rust_toolchain_info = ctx.attrs._rust_toolchain[RustToolchainInfo]

    # The script runs in the package's directory inside a copy of the manifest tree, so that a
    # relative path out of the package, such as `../proto/api.proto`, resolves as with Cargo.
    subdir = ctx.attrs.manifest_subdir
    cwd = ctx.actions.declare_output("cwd/" + subdir if subdir else "cwd", dir = True, has_content_based_path = True)
    out_dir = ctx.actions.declare_output("OUT_DIR", dir = True, has_content_based_path = True)
    rustc_flags = ctx.actions.declare_output("rustc_flags", has_content_based_path = True)
    linker_flags = ctx.actions.declare_output("linker_flags", has_content_based_path = True)
    shared_libs = ctx.actions.declare_output("shared_libs", dir = True, has_content_based_path = True)
    metadata = ctx.actions.declare_output("metadata", has_content_based_path = True)

    if ctx.attrs.manifest_dir != None:
        if ctx.attrs.package_srcs or subdir:
            fail("`package_srcs` and `manifest_subdir` take the files of `filegroup_for_manifest_dir`, not of `manifest_dir`")
        manifest_dir = ctx.attrs.manifest_dir[DefaultInfo].default_outputs[0]
    else:
        manifest_srcs = dict(ctx.attrs.filegroup_for_manifest_dir)
        manifest_srcs.update(package_srcs_files(ctx.attrs.package_srcs))
        manifest_dir = ctx.actions.symlinked_dir("manifest_dir", manifest_srcs, has_content_based_path = True)

    cmd = [
        ctx.attrs.runner[RunInfo],
        cmd_args("--buildscript=", ctx.attrs.buildscript[RunInfo], delimiter = ""),
        cmd_args("--rustc-cfg=", ctx.attrs.rustc_cfg[DefaultInfo].default_outputs[0], delimiter = ""),
        cmd_args("--manifest-dir=", manifest_dir, delimiter = ""),
        "--manifest-subdir=" + subdir,
        cmd_args("--create-cwd=", cwd.as_output(), delimiter = ""),
        cmd_args("--outfile=", rustc_flags.as_output(), delimiter = ""),
        cmd_args("--linker-flags=", linker_flags.as_output(), delimiter = ""),
        cmd_args("--shared-libs=", shared_libs.as_output(), delimiter = ""),
        cmd_args("--metadata=", metadata.as_output(), delimiter = ""),
    ]
    for dep in ctx.attrs.links_deps:
        info = dep[BuildScriptMetadataInfo]
        cmd.append(cmd_args("--dep-metadata", info.metadata, info.out_dir, info.manifest_dir))
    linker_search_flag = "-L"
    if cxx_toolchain_info.linker_info.type == LinkerType("windows"):
        linker_search_flag = "/LIBPATH:"
    cmd.append("--linker-search-flag=" + linker_search_flag)

    if ctx.attrs.rustc_link_lib:
        cmd.append("--rustc-link-lib")
    if ctx.attrs.rustc_link_search:
        cmd.append("--rustc-link-search")

    # See https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts

    env = {}
    env["CARGO"] = "/bin/false"
    env["CARGO_PKG_NAME"] = ctx.attrs.package_name
    env["CARGO_PKG_VERSION"] = ctx.attrs.version
    release, _, pre = ctx.attrs.version.partition("-")
    version_parts = (release.split(".") + ["", "", ""])[:3]
    env["CARGO_PKG_VERSION_MAJOR"] = version_parts[0]
    env["CARGO_PKG_VERSION_MINOR"] = version_parts[1]
    env["CARGO_PKG_VERSION_PATCH"] = version_parts[2]
    env["CARGO_PKG_VERSION_PRE"] = pre
    env["OUT_DIR"] = out_dir.as_output()
    env["RUSTC"] = _make_rustc_shim(ctx, cwd)
    if rust_toolchain_info.rustdoc:
        env["RUSTDOC"] = cmd_args(rust_toolchain_info.rustdoc, delimiter = " ", relative_to = cwd)
    env["RUSTC_LINKER"] = "/bin/false"
    env["RUST_BACKTRACE"] = "1"

    if rust_toolchain_info.rustc_target_triple:
        env["TARGET"] = rust_toolchain_info.rustc_target_triple
    else:
        cmd.append(cmd_args("--rustc-host-tuple=", ctx.attrs.rustc_host_tuple[DefaultInfo].default_outputs[0], delimiter = ""))

    # \037 == \x1f == the magic delimiter specified in the environment variable
    # reference above.
    env["CARGO_ENCODED_RUSTFLAGS"] = cmd_args(
        rust_toolchain_info.rustc_flags,
        "-Cunsafe-allow-abi-mismatch=sanitizer",
        delimiter = "\037",
    )

    host_triple = targets.exec_triple(ctx)
    if host_triple:
        env["HOST"] = host_triple

    for feature in ctx.attrs.features:
        upper_feature = feature.upper().replace("-", "_")
        env["CARGO_FEATURE_{}".format(upper_feature)] = "1"

    # Cargo always sets these, and some build scripts read them without a fallback.
    opt_level = "0"
    debug = "false"
    for flag in rust_toolchain_info.rustc_flags:
        if isinstance(flag, ResolvedStringWithMacros):
            flag = str(flag)[1:-1]
        if flag.startswith("-Copt-level="):
            opt_level = flag.removeprefix("-Copt-level=")
        elif flag == "-g" or (flag.startswith("-Cdebuginfo=") and flag != "-Cdebuginfo=0"):
            debug = "true"
    env["OPT_LEVEL"] = opt_level
    env["PROFILE"] = "debug" if opt_level == "0" else "release"
    env["DEBUG"] = debug

    # The number of jobs a build script may run. One keeps the script's own
    # parallelism from competing with the actions yak runs at once.
    env["NUM_JOBS"] = "1"

    # C and C++ compilers for bindgen.
    target_triple = get_target_triple(ctx)
    preprocessor = cxx_merge_cpreprocessors(
        ctx.actions,
        [],
        cxx_inherited_preprocessor_infos(ctx.attrs.cxx_deps),
    )
    deps_preprocessor_flags = preprocessor.set.project_as_args("args")
    deps_tset = ctx.actions.tset(
        LinkInfosTSet,
        children = [dep[MergedLinkInfo]._infos[LinkStrategy("static_pic")] for dep in ctx.attrs.cxx_deps],
    )
    deps_link = deps_tset.project_as_args("default")
    sanitizer_flags = ["-fno-sanitize=all"]
    cc_is_clang = cxx_toolchain_info.c_compiler_info.compiler_type.startswith("clang")
    cxx_is_clang = cxx_toolchain_info.cxx_compiler_info.compiler_type.startswith("clang")
    # `LD` runs the toolchain's linker driver with each argument passed through as `-Wl,<arg>`,
    # so it stands in for a plain linker. The arguments already hold the startup files, the
    # libraries, and the output kind of the link, which the driver would add again. Without
    # `-nostdlib` and `-no-pie`, a driver that defaults to PIE, such as Debian's clang, links
    # its own startup files and adds `-pie` to a `-shared` link, which `ld.lld` rejects.
    gnu_driver_flags = ["-nostdlib", "-no-pie"] if cxx_toolchain_info.linker_info.type == LinkerType("gnu") else []
    env["LD"] = _make_cc_shim(
        ctx = ctx,
        name = "__ld_shim",
        cmd = cmd_args(
            cxx_toolchain_info.linker_info.linker,
            cxx_toolchain_info.linker_info.linker_flags or [],
            rust_toolchain_info.linker_flags,
            gnu_driver_flags,
            sanitizer_flags,
        ),
    )
    env["CC"] = _make_cc_shim(
        ctx = ctx,
        name = "__cc_shim",
        cmd = cmd_args(
            cxx_toolchain_info.c_compiler_info.compiler,
            cmd_args(env["LD"], format = "--ld-path={}") if cc_is_clang else cmd_args(),
            cxx_toolchain_info.c_compiler_info.preprocessor_flags,
            cxx_toolchain_info.c_compiler_info.compiler_flags,
            deps_preprocessor_flags,
            deps_link,
            sanitizer_flags,
            ctx.attrs.cxx_flags,
            ["--target={}".format(target_triple)] if target_triple else [],
        ),
    )
    env["CXX"] = _make_cc_shim(
        ctx = ctx,
        name = "__cxx_shim",
        cmd = cmd_args(
            cxx_toolchain_info.cxx_compiler_info.compiler,
            cmd_args(env["LD"], format = "--ld-path={}") if cxx_is_clang else cmd_args(),
            cxx_toolchain_info.cxx_compiler_info.preprocessor_flags,
            cxx_toolchain_info.cxx_compiler_info.compiler_flags,
            deps_preprocessor_flags,
            deps_link,
            sanitizer_flags,
            ctx.attrs.cxx_flags,
            ["--target={}".format(target_triple)] if target_triple else [],
        ),
    )
    env["AR"] = _make_cc_shim(
        ctx = ctx,
        name = "__ar_shim",
        cmd = cmd_args(cxx_toolchain_info.linker_info.archiver),
    )

    # Environment variables specified in the target's attributes get priority
    # over all the above.
    for k, v in ctx.attrs.env.items():
        env[k] = cmd_args(v, relative_to = cwd)

    ctx.actions.run(
        cmd,
        env = env,
        category = "buildscript",
    )

    # A library that depends on this target passes these flags to every link
    # that includes it, because rustc does not record the library's search
    # paths in its metadata. `linker_flags` holds the directories of the host
    # libraries that the script names, and `shared_libs` holds the shared
    # libraries of its `OUT_DIR`.
    link = LinkInfo(
        name = ctx.attrs.name,
        pre_flags = [
            cmd_args(linker_flags, format = "@{}"),
            cmd_args(shared_libs, format = linker_search_flag + "{}"),
        ],
    )
    return [
        DefaultInfo(
            default_output = None,
            sub_targets = {
                "linker_flags": [DefaultInfo(default_output = linker_flags)],
                "metadata": [DefaultInfo(default_output = metadata)],
                "out_dir": [DefaultInfo(default_output = out_dir)],
                "rustc_flags": [DefaultInfo(default_output = rustc_flags)],
                "shared_libs": [DefaultInfo(default_output = shared_libs)],
            },
        ),
        BuildScriptMetadataInfo(metadata = metadata, out_dir = out_dir, manifest_dir = manifest_dir),
        # The binaries that depend on this target load the shared libraries
        # that the script linked from its `OUT_DIR`.
        BuildScriptSharedLibsInfo(
            dirs = ctx.actions.tset(BuildScriptSharedLibDirs, value = shared_libs),
        ),
        EMPTY_SHARED_LIBRARY_INFO,
        EMPTY_LINK_GROUP_LIB_INFO,
        create_linkable_graph(ctx),
        create_merged_link_info(
            ctx,
            cxx_toolchain_info.pic_behavior,
            {output_style: LinkInfos(default = link) for output_style in LibOutputStyle},
        ),
    ]

_cargo_buildscript_rule = rule(
    impl = _cargo_buildscript_impl,
    attrs = {
        "buildscript": attrs.exec_dep(providers = [RunInfo]),
        "cxx_deps": attrs.list(attrs.dep(), default = []),
        "cxx_flags": attrs.list(attrs.arg(), default = []),
        "env": attrs.dict(key = attrs.string(), value = attrs.arg(), default = {}),
        "features": attrs.list(attrs.string(), default = []),
        "filegroup_for_manifest_dir": attrs.option(attrs.dict(key = attrs.string(), value = attrs.source()), default = None),
        # The run targets of the build scripts of the package's normal dependencies with `links`,
        # whose metadata the script gets as `DEP_<links>_<key>`.
        "links_deps": attrs.list(attrs.dep(providers = [BuildScriptMetadataInfo]), default = []),
        "manifest_dir": attrs.option(attrs.dep(), default = None),
        # The directory of the package in the tree of `filegroup_for_manifest_dir`, where the
        # script runs and which `CARGO_MANIFEST_DIR` names.
        "manifest_subdir": attrs.string(default = ""),
        "package_name": attrs.string(),
        # The files of targets in other packages that the script reads, each placed below a
        # directory of the tree of `filegroup_for_manifest_dir`, as in `package_srcs` of the Rust
        # rules.
        "package_srcs": attrs.dict(key = attrs.dep(), value = attrs.string(), sorted = False, default = {}),
        "runner": attrs.default_only(attrs.exec_dep(providers = [RunInfo], default = "prelude//rust/tools:buildscript_run")),
        # *IMPORTANT* rustc_cfg must be a `dep` and not an `exec_dep` because
        # we want the `rustc --cfg` for the target platform, not the exec platform.
        "rustc_cfg": attrs.dep(default = "prelude//rust/tools:rustc_cfg"),
        "rustc_host_tuple": attrs.dep(default = "prelude//rust/tools:rustc_host_tuple"),
        "rustc_link_lib": attrs.bool(default = False),
        "rustc_link_search": attrs.bool(default = False),
        "version": attrs.string(),
        "_cxx_toolchain": toolchains_common.cxx(),
        "_exec_os_type": yak.exec_os_type_arg(),
        "_rust_internal_tools_toolchain": attrs.default_only(
            attrs.toolchain_dep(default = "prelude//rust/tools:internal_tools_toolchain"),
        ),
        "_rust_toolchain": toolchains_common.rust(),
    },
    # Always empty, but needed to prevent errors
    uses_plugins = [RustProcMacroPlugin],
)

def buildscript_run(
    name,
    buildscript_rule,
    package_name,
    version,
    platform = {},
    # path to crate's directory in source tree, e.g. "vendor/serde-1.0.100"
    local_manifest_dir = None,
    # target or subtarget containing crate, e.g. ":serde.git[serde]"
    manifest_dir = None,
    buildscript_compatible_with = None,
    **kwargs,
):
    kwargs = apply_platform_attrs(platform, kwargs)

    filegroup_for_manifest_dir = kwargs.pop("filegroup_for_manifest_dir", None)
    if manifest_dir == None and local_manifest_dir == None and filegroup_for_manifest_dir == None:
        existing_filegroup_name = "{}-{}.crate".format(package_name, version)
        if rule_exists(existing_filegroup_name):
            manifest_dir = ":{}".format(existing_filegroup_name)
        else:
            local_manifest_dir = "vendor/{}-{}".format(package_name, version)

    if local_manifest_dir != None:
        prefix_with_trailing_slash = "{}/".format(local_manifest_dir)
        filegroup_for_manifest_dir = {path.removeprefix(prefix_with_trailing_slash): path for path in glob(["{}/**".format(local_manifest_dir)])}

    def platform_buildscript_build_name(plat):
        if name.endswith("-build-script-run"):
            # This is the expected case for generated third-party targets, which
            # come in pairs build-script-run and build-script-build.
            return "{}-build-script-build-{}".format(
                name.removesuffix("-build-script-run"),
                plat,
            )
        else:
            return "{}-{}".format(name, plat)

    for i, plat in enumerate(get_cargo_platform_names()):
        transition_alias(
            name = platform_buildscript_build_name(plat),
            actual = buildscript_rule,
            incoming_transition = "prelude//rust/buildscript:buildscript_platform_transition[{}]".format(i),
            target_compatible_with = buildscript_compatible_with,
            visibility = [],
        )

    buildscript_rule = selects.apply(
        get_cargo_platforms(),
        lambda plat: buildscript_rule if plat == None else ":{}".format(platform_buildscript_build_name(plat)),
    )

    _cargo_buildscript_rule(
        name = name,
        buildscript = buildscript_rule,
        package_name = package_name,
        version = version,
        filegroup_for_manifest_dir = filegroup_for_manifest_dir,
        manifest_dir = manifest_dir,
        **kwargs,
    )
