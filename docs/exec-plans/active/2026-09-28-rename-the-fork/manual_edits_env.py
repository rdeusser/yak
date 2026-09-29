#!/usr/bin/env python3
"""Hand edits of milestone 3 that follow rename_env.py.

Run from the repository root after `rename_env.py --apply`:

    python3 path/to/manual_edits_env.py

Each edit names its file, the exact text it replaces, and how many times that
text occurs, so a rerun on a changed tree fails instead of editing blindly.
"""

import sys

failures = []


def edit(path, old, new, count=1):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    n = text.count(old)
    if n != count:
        failures.append(f"{path}: expected {count} of {old!r}, found {n}")
        return
    with open(path, "w", encoding="utf-8") as f:
        f.write(text.replace(old, new))


# The flag took its name from the `no_buckd` field. The comment compared the
# variable with Buck1's `NO_BUCKD`.
edit(
    "app/buck2/src/lib.rs",
    "    /// Note even when running in no-buckd mode, it still writes state files.\n",
    "    /// Note even when running in no-yakd mode, it still writes state files.\n",
)
edit(
    "app/buck2/src/lib.rs",
    """    #[clap(env("YAK_NO_YAKD"), long, global(true), hide(true))]
    // Env var is YAK_NO_YAKD instead of NO_BUCKD env var from buck1 because no yakd
    // is not supported for production work for yak and lots of places already set
    // NO_BUCKD=1 for buck1.
    no_buckd: bool,
""",
    """    #[clap(env("YAK_NO_YAKD"), long = "no-yakd", global(true), hide(true))]
    no_buckd: bool,
""",
)

# `host_info().buck2` told Buck2 from Buck1, and yak has no Buck1.
edit(
    "app/buck2_interpreter_for_build/src/interpreter/functions/host_info.rs",
    """                ("arch", arch),
                // TODO(cjhopman): Remove in favour of version_info() in Buck v1 and v2
                // We want to be able to determine if we are on Buck v2 or not, this mechanism
                // is quick, cheap and Buck v1 compatible.
                ("buck2", Value::new_bool(true)),
""",
    """                ("arch", arch),
""",
)
edit(
    "app/buck2_interpreter_for_build_tests/src/functions/host_info.rs",
    """#[test]
fn test_buck_v2() -> buck2_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test(indoc!(
        r#"
            def test():
                assert_eq(True, hasattr(host_info(), "buck2"))
                assert_eq(False, hasattr(host_info(), "buck1"))
        "#
""",
    """#[test]
fn test_no_buck_version_fields() -> buck2_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test(indoc!(
        r#"
            def test():
                assert_eq(False, hasattr(host_info(), "buck2"))
                assert_eq(False, hasattr(host_info(), "buck1"))
        "#
""",
)

# `default_remote_execution_use_case` is in the `build` section
# (`DEFAULT_RE_USE_CASE_KEY` in app/buck2_execute/src/re/invocation_re_settings.rs).
edit(
    "app/buck2_server/src/ctx.rs",
    "or a global default (yak.default_remote_execution_use_case).",
    "or a global default (build.default_remote_execution_use_case).",
)

# After the rename, the comparison with Buck1 named Buck1's variables by their
# yak names.
edit(
    "prelude/sh_binary.bzl",
    """            # In buck1, the paths for resources that are outputs of rules have
            # different paths in YAK_PROJECT_ROOT and
            # YAK_DEFAULT_RUNTIME_RESOURCES, but we use the same paths. buck1's
            # YAK_PROJECT_ROOT paths would use the actual yak-out path rather
            # than something derived from the target and so to use that people
            # would need to hardcode yak-out paths into their scripts. For repo
            # sources, the paths are the same for both.
""",
    """            # YAK_PROJECT_ROOT and YAK_DEFAULT_RUNTIME_RESOURCES name the same
            # directory. A resource that a rule outputs has a path there that
            # derives from its target, so scripts need no yak-out paths.
""",
)

# `cargo fmt` joins this call onto one line after the rename shortens the name,
# and it keeps the trailing comma there.
edit(
    "app/buck2_common/src/init.rs",
    """            "YAK_TEST_RESOURCE_CONTROL_CONFIG",
            applicability = testing,
        )? {""",
    """            "YAK_TEST_RESOURCE_CONTROL_CONFIG",
            applicability = testing
        )? {""",
)

# The values of a config file come ordered by section, and `yak` sorts after
# the test's own section where `buck2` sorted before it.
edit(
    "tests/core/build/test_external_buckconfigs.py",
    """    # Our tests inject file_watcher to external configs in test setup stage
    local_config_value = external_path_configs["values"][external_index]
    assert (
        local_config_value["section"] == "yak"
        and local_config_value["key"] == "file_watcher"
        and local_config_value["value"] == "fs_hash_crawler"
        and not local_config_value["is_cli"]
    )
    external_index += 1
    external_path_config_value = external_path_configs["values"][external_index]
    assert (
        external_path_config_value["section"] == "external_path_configs_section"
        and external_path_config_value["key"] == "external_path_configs_key"
        and external_path_config_value["value"] == "external_path_configs_value"
        and not external_path_config_value["is_cli"]
    )
""",
    """    # Yak orders the values of a config file by section, so the test's own
    # section comes before the `yak` section that the test setup injects.
    external_path_config_value = external_path_configs["values"][external_index]
    assert (
        external_path_config_value["section"] == "external_path_configs_section"
        and external_path_config_value["key"] == "external_path_configs_key"
        and external_path_config_value["value"] == "external_path_configs_value"
        and not external_path_config_value["is_cli"]
    )
    external_index += 1
    # Our tests inject file_watcher to external configs in test setup stage
    local_config_value = external_path_configs["values"][external_index]
    assert (
        local_config_value["section"] == "yak"
        and local_config_value["key"] == "file_watcher"
        and local_config_value["value"] == "fs_hash_crawler"
        and not local_config_value["is_cli"]
    )
""",
)

# The status lines name the work that remains.
edit(
    "AGENTS.md",
    "tracks the remaining work, such as the `BUCK2_` environment variables and the `buck2*` crates.",
    "tracks the remaining work, such as the Java packages and the `buck2*` crates.",
)
edit(
    "ARCHITECTURE.md",
    "- The rename to yak continues with the `BUCK2_` environment variables, the `[yak]` configuration sections, the Java packages, and the `buck2*` crates.",
    "- The rename to yak continues with the Java packages and the `buck2*` crates.",
)

edit(
    "CHANGELOG.md",
    """- Wheels that `python_wheel` builds name `yak` as their generator in the `WHEEL` file.
""",
    """- Wheels that `python_wheel` builds name `yak` as their generator in the `WHEEL` file.
- The environment variables that yak reads take the `YAK_` prefix in place of `BUCK2_` and `BUCK_`, such as `YAK_LOG` and `YAK_ISOLATION_DIR`.
- The daemon startup timeouts are `YAKD_STARTUP_TIMEOUT` and `YAKD_STARTUP_INIT_TIMEOUT`.
- Actions and the prelude's tools see `YAK_SCRATCH_PATH`, `YAK_BUILD_ID`, and the other variables that yak and the prelude set, in place of their `BUCK_` names.
- The configuration sections are `[yak]`, `[yak_re_client]`, `[yak_resource_control]`, `[yak_system_warning]`, `[yak_hydration]`, and `[yak_metadata]`.
- The hidden flag that runs the daemon in the client process is `--no-yakd`, and its variable is `YAK_NO_YAKD`.
- `host_info()` no longer has a `buck2` field.
- The integration tests take the binary from `YAK_BINARY` and rewrite golden files when `YAK_UPDATE_GOLDEN` is set.
""",
)

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("manual edits applied")
