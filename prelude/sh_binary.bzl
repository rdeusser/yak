# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//:paths.bzl", "paths")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup")

def _derive_link(artifact):
    if artifact.is_source:
        return artifact.short_path

    # TODO(cjhopman): Reject cross-repo resources. It's probably possible to
    # construct a scheme for them that is correct, but not necessary yet.

    return paths.join(artifact.owner.package, artifact.owner.name)

def _generate_script(
    name: str,
    main: Artifact,
    resources: list[Artifact],
    append_script_extension: bool,
    actions: AnalysisActions,
    is_windows: bool,
    copy_resources: bool,
    has_content_based_path: bool,
) -> (Artifact, Artifact):
    main_path = main.short_path
    if not append_script_extension:
        main_link = main_path
    elif is_windows:
        main_link = main_path if main_path.endswith(".bat") or main_path.endswith(".cmd") else main_path + ".bat"
    else:
        main_link = main_path if main_path.endswith(".sh") else main_path + ".sh"
    resources = {_derive_link(src): src for src in resources}
    resources[main_link] = main

    # windows isn't stable with resources passed in as symbolic links for
    # remote execution. Allow using copies instead.
    if copy_resources:
        resources_dir = actions.copied_dir("resources", resources, has_content_based_path = has_content_based_path)
    else:
        resources_dir = actions.symlinked_dir("resources", resources, has_content_based_path = has_content_based_path)

    script_name = name + (".bat" if is_windows else "")
    script = actions.declare_output(script_name, has_content_based_path = has_content_based_path)

    # The script stays simple:
    # 1. we don't invoke the script through a symlink and so don't need to use and implement a cross-platform `readlink -e`
    # 2. we don't construct an invocation-specific sandbox.
    # 3. we don't construct cell symlinks. Instead we construct links directly to things.
    if not is_windows:
        script_content = cmd_args(
            "#!/usr/bin/env bash",
            "set -e",
            # If we access this sh_binary via a unhashed symlink we need to
            # update the relative source.
            '__SRC="${BASH_SOURCE[0]}"',
            '__SRC="$(realpath "$__SRC")"',
            '__SCRIPT_DIR=$(dirname "$__SRC")',
            cmd_args('export YAK_PROJECT_ROOT="$__SCRIPT_DIR/', resources_dir, '"', delimiter = ""),
            # Normalize backslashes to forward slashes for the Windows-host /
            # Linux-target (RE) case where relative_to produces Windows-style separators.
            'export YAK_PROJECT_ROOT="${YAK_PROJECT_ROOT//\\\\//}"',
            # YAK_PROJECT_ROOT and YAK_DEFAULT_RUNTIME_RESOURCES name the same
            # directory. A resource that a rule outputs has a path there that
            # derives from its target, so scripts need no yak-out paths.
            'export YAK_DEFAULT_RUNTIME_RESOURCES="$YAK_PROJECT_ROOT"',
            'exec "$YAK_PROJECT_ROOT/{}" "$@"'.format(main_link),
            relative_to = (script, 1),
        )
    else:
        script_content = cmd_args(
            "@echo off",
            "setlocal EnableDelayedExpansion",
            # Fully qualified script path.
            "set __SRC=%~f0",
            # Symbolic links on windows RE systems may not be stable depending on implementation.
            # Don't try to resolve the links when using copies.
            'for /f "tokens=2 delims=[]" %%a in (\'dir %__SRC% ^|%SYSTEMROOT%\\System32\\find.exe "<SYMLINK>"\') do set "__SRC=%%a"'
            if not copy_resources
            else "",
            # Get parent folder.
            'for %%a in ("%__SRC%") do set "__SCRIPT_DIR=%%~dpa"',
            cmd_args("set YAK_PROJECT_ROOT=%__SCRIPT_DIR%\\", resources_dir, delimiter = ""),
            "set YAK_DEFAULT_RUNTIME_RESOURCES=%YAK_PROJECT_ROOT%",
            "%YAK_PROJECT_ROOT%\\{} %*".format(main_link),
            relative_to = (script, 1),
        )
    actions.write(
        script,
        script_content,
        is_executable = True,
    )

    return (script, resources_dir)

# Attrs:
# "deps": attrs.list(attrs.dep(), default = []),
# "main": attrs.source(),
# "resources": attrs.list(attrs.source(), default = []),
# "copy_resources": attrs.bool(default = False),
def sh_binary_impl(ctx):
    # TODO: implement deps (not sure what those even do, though)
    if len(ctx.attrs.deps) > 0:
        fail("sh_binary deps unsupported. Got `{}`".format(repr(ctx.attrs)))

    is_windows = ctx.attrs._target_os_type[OsLookup].os == Os("windows")
    (script, resources_dir) = _generate_script(
        ctx.label.name,
        ctx.attrs.main,
        ctx.attrs.resources,
        ctx.attrs.append_script_extension,
        ctx.actions,
        is_windows,
        ctx.attrs.copy_resources,
        ctx.attrs.has_content_based_path,
    )

    script = script.with_associated_artifacts([resources_dir])

    return [
        DefaultInfo(default_output = script, other_outputs = [resources_dir]),
        RunInfo(
            # TODO(cjhopman): Figure out if we need to specify the link targets
            # as inputs. We shouldn't need to, but need to verify it.
            args = cmd_args(script, hidden = resources_dir),
        ),
    ]
