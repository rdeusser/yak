# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//tests:remote_test_execution_toolchain.bzl", "RemoteTestExecutionToolchainInfo")
load("@prelude//utils:expect.bzl", "expect_non_none")
load("@prelude//utils:type_defs.bzl", "type_utils")

ReArg = record(
    disabled = field(bool | None, default = None),
    re_props = field(dict | None, default = None),
)

# Result of `get_re_executors_from_props`.
#
# `run_from_project_root` and `use_project_relative_paths` report whether the
# returned executor is remote-execution eligible and therefore the test must run
# from the project root with project-relative paths. They are currently the same
# value, but keep them separate from each other and from "is there an executor at
# all?" so callers can evolve cwd and path behavior independently.
RemoteTestExecutorConfig = record(
    default_executor = field([CommandExecutorConfig, None], default = None),
    executor_overrides = field(dict[str, CommandExecutorConfig], default = {}),
    run_from_project_root = field(bool, default = False),
    use_project_relative_paths = field(bool, default = False),
)

def _network_access_kwargs(network_access: str | None) -> dict[str, str]:
    if network_access == None:
        return {}
    return {"network_access": network_access}

def _force_local_re_tests() -> bool:
    # `read_bool` is not usable here because its `read_config` binding is unavailable
    # during analysis, so this mirrors its coercion, including rejecting values it
    # cannot coerce.
    value = read_root_config("tests", "disable_re_tests", "")

    # An empty value means "unset", so that a later config can clear an earlier one.
    if value == "":
        return False
    lowered = value.lower()
    if lowered == "true":
        return True
    if lowered == "false":
        return False
    fail("`tests.disable_re_tests`: cannot coerce {!r} to bool".format(value))

def _get_re_arg(ctx: AnalysisContext) -> ReArg:
    if _force_local_re_tests() or not hasattr(ctx.attrs, "remote_execution"):
        # This path is also taken when the attribute is missing entirely.
        # `-c tests.disable_re_tests=true` lets users run tests locally, for example
        # `yak test --local-only -c tests.disable_re_tests=true //path/to:test`.
        return ReArg(re_props = None)

    if ctx.attrs.remote_execution != None:
        if ctx.attrs.remote_execution == "disabled":
            return ReArg(disabled = True)
        elif type_utils.is_string(ctx.attrs.remote_execution):
            # If this is a string, look up the re_props on the RE toolchain.
            expect_non_none(ctx.attrs._remote_test_execution_toolchain)
            return ReArg(
                re_props = ctx.attrs._remote_test_execution_toolchain[RemoteTestExecutionToolchainInfo].profiles[ctx.attrs.remote_execution],
            )

        return ReArg(re_props = ctx.attrs.remote_execution)

    # Check for a default RE option on the toolchain.
    re_toolchain = ctx.attrs._remote_test_execution_toolchain
    if re_toolchain != None and re_toolchain[RemoteTestExecutionToolchainInfo].default_profile != None:
        return ReArg(re_props = re_toolchain[RemoteTestExecutionToolchainInfo].default_profile)

    return ReArg(re_props = None)

def get_re_executors_from_props(ctx: AnalysisContext) -> RemoteTestExecutorConfig:
    """
    Convert the `remote_execution` properties param into `CommandExecutorConfig` objects to use with test providers.

    The target's `network_access` policy (if any) is attached to the returned
    `CommandExecutorConfig`(s) so it is enforced for both local and remote test
    execution. When a target has no RE profile, a local-only executor is synthesized
    so Buck does not fall back to a remote-only build execution platform for a test
    whose cell-relative paths make it ineligible for RE. The executor is reported with
    `run_from_project_root = False` so the test keeps running in-place rather than
    being switched to project-root/RE-style execution.

    Returns a `RemoteTestExecutorConfig`.
    """

    return _get_re_executors(ctx, _get_re_arg(ctx))

def get_re_executors_from_explicit_props(ctx: AnalysisContext, re_props: dict | None) -> RemoteTestExecutorConfig:
    """
    Like `get_re_executors_from_props`, but for a rule that carries RE properties in
    some attribute other than `remote_execution`.

    `re_props` must already be a resolved property dict. The string form of
    `remote_execution` — the name of a profile on the remote test execution toolchain —
    is deliberately not accepted here: resolving one requires
    `_remote_test_execution_toolchain`, and a rule that runs RE only for a secondary
    test (a rustdoc doctest, say) should not have to take a toolchain dependency to
    express that.

    Returns a `RemoteTestExecutorConfig`.
    """

    if _force_local_re_tests():
        re_props = None

    return _get_re_executors(ctx, ReArg(re_props = re_props))

def _get_re_executors(ctx: AnalysisContext, re_arg: ReArg) -> RemoteTestExecutorConfig:
    network_access = getattr(ctx.attrs, "network_access", None)

    if re_arg.disabled:
        executor = CommandExecutorConfig(local_enabled = True, remote_enabled = False, **_network_access_kwargs(network_access))
        # A `remote_execution = "disabled"` target has always produced an executor
        # and therefore run from the project root; preserve that behavior.
        return RemoteTestExecutorConfig(default_executor = executor, run_from_project_root = True, use_project_relative_paths = True)

    re_props = re_arg.re_props
    if re_props == None:
        # A test without an RE profile uses cell-relative paths, which makes the test
        # action local-only. Give it an explicit local executor instead of inheriting
        # the rule's build execution platform: that platform may be remote-only when
        # cross-building, even though the target binary must run on the local host.
        #
        # `remote_cache_enabled = False` keeps this a plain `Executor::Local`, and
        # nothing is uploaded from here anyway (`allow_cache_uploads` is False).
        executor = CommandExecutorConfig(local_enabled = True, remote_enabled = False, remote_cache_enabled = False, **_network_access_kwargs(network_access))
        return RemoteTestExecutorConfig(default_executor = executor)

    re_props_copy = dict(re_props)
    missing_props = [name for name in ("capabilities", "use_case") if name not in re_props_copy]
    if missing_props:
        fail("{}: re props are missing required fields: {}".format(ctx.label, ", ".join(missing_props)))
    capabilities = re_props_copy.pop("capabilities")
    use_case = re_props_copy.pop("use_case")
    listing_capabilities = re_props_copy.pop("listing_capabilities", None)
    remote_cache_enabled = re_props_copy.pop("remote_cache_enabled", None)
    local_enabled = re_props_copy.pop("local_enabled", False)
    local_listing_enabled = re_props_copy.pop("local_listing_enabled", None)
    re_resource_units = re_props_copy.pop("resource_units", None)
    re_listing_resource_units = re_props_copy.pop("listing_resource_units", re_resource_units)
    if re_props_copy:
        unexpected_props = ", ".join(re_props_copy.keys())
        fail("{}: found unexpected re props: {}".format(ctx.label, unexpected_props))

    default_executor = CommandExecutorConfig(
        local_enabled = local_enabled,
        remote_enabled = True,
        remote_execution_properties = capabilities,
        remote_execution_use_case = use_case,
        remote_cache_enabled = remote_cache_enabled,
        remote_execution_resource_units = re_resource_units,
        **_network_access_kwargs(network_access),
    )

    listing_executor = default_executor
    if listing_capabilities != None or local_listing_enabled != None:
        listing_executor = CommandExecutorConfig(
            local_enabled = local_listing_enabled if local_listing_enabled != None else local_enabled,
            remote_enabled = True,
            remote_execution_properties = listing_capabilities if listing_capabilities != None else capabilities,
            remote_execution_use_case = use_case,
            remote_cache_enabled = remote_cache_enabled,
            remote_execution_resource_units = re_listing_resource_units,
            **_network_access_kwargs(network_access),
        )
    return RemoteTestExecutorConfig(
        default_executor = default_executor,
        executor_overrides = {"listing": listing_executor},
        run_from_project_root = True,
        use_project_relative_paths = True,
    )
