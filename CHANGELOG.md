# Changelog

## Unreleased

Removes the code, configuration, and service clients that only Meta's internal build used, removes the JVM, Android, and JavaScript support and the Buck1 compatibility code, and renames the tool to yak.

### Renamed to yak

- The binary is `yak`, and `cargo build --bin=yak` builds it.
- yak reads build files named `YAK`.
- `[buildfile] name` lists the exact build file names to read. The `name_v2` key and the `.v2` variant of each name are gone.
- The project configuration files are `.yakconfig`, `.yakconfig.local`, and `.yakconfig.d/`.
- The global configuration is in `/etc/yakconfig` and `/etc/yakconfig.d/`, or in `C:\ProgramData\yakconfig` and `C:\ProgramData\yakconfig.d` on Windows.
- A `.yakroot` file marks the project root.
- The settings files are `.yaksettings.toml` and `.yaksettings.local.toml`.
- Build output goes to `yak-out`.
- The reserved directory in `yak-out` is `._yak`. Tools keep scratch files in `yak-out/._yak/tmp`, and `--isolation-dir` rejects names that start with `._yak`.
- The daemon keeps its state in `~/.yak/yakd/`, in files named `yakd.info`, `yakd.pid`, and so on.
- The alternative package file name is `YAK_TREE` in place of `BUCK_TREE`.
- yak reads none of the old names. A project renames its `BUCK`, `.buckconfig`, and `.buckroot` files, or sets `[buildfile] name = BUCK` in `.yakconfig` to keep its build files.
- For a crate whose target is incompatible with the host, `prelude//rust/rust-analyzer/resolve_deps.bxl` reports the directory of its build file as the source folder. It removed only a `/TARGETS` or `/BUCK` file name before. It also joined the cell's name to the project root in place of the cell's path.
- The daemon runs in the systemd slice `yak.slice` as the unit `yak-daemon.<project>.<isolation dir>.<id>`.
- The daemon's process title is `yakd[<project>]`.
- `yak killall` and `yak clean --stale` look for processes named `yak` and `yak-daemon`.
- The client-only build looks for the daemon binary `yak-daemon` next to the client.
- Shell completions register for `yak` and no longer for a `buck` command.
- Remote Execution requests name the tool `yak`.
- The release assets are named `yak-<target triple>`.
- Wheels that `python_wheel` builds name `yak` as their generator in the `WHEEL` file.
- The environment variables that yak reads take the `YAK_` prefix in place of `BUCK2_` and `BUCK_`, such as `YAK_LOG` and `YAK_ISOLATION_DIR`.
- The daemon startup timeouts are `YAKD_STARTUP_TIMEOUT` and `YAKD_STARTUP_INIT_TIMEOUT`.
- Actions and the prelude's tools see `YAK_SCRATCH_PATH`, `YAK_BUILD_ID`, and the other variables that yak and the prelude set, in place of their `BUCK_` names.
- The configuration sections are `[yak]`, `[yak_re_client]`, `[yak_resource_control]`, `[yak_system_warning]`, `[yak_hydration]`, and `[yak_metadata]`.
- The hidden flag that runs the daemon in the client process is `--no-yakd`, and its variable is `YAK_NO_YAKD`.
- `host_info()` no longer has a `buck2` field.
- The integration tests take the binary from `YAK_BINARY` and rewrite golden files when `YAK_UPDATE_GOLDEN` is set.
- The documentation calls the tool yak and its configuration the yakconfig.
- The site's pages `concepts/buckconfig`, `concepts/buck_out`, `concepts/buck_query_language`, `getting_started/what_is_buck2`, and `users/faq/buck_hanging` moved to `concepts/yakconfig`, `concepts/yak_out`, `concepts/query_language`, `getting_started/what_is_yak`, and `users/faq/yak_hanging`.
- The site no longer has the page that compared Buck2 with Buck1, or the lists of articles, videos, projects, and tools about Buck2.
- The site's logo is a yak.
- `website/gen_docs.py` takes the path of the binary with `--yak` in place of `--buck2`.
- The publisher of the Starlark extension for VS Code is `yak`.

### Removed JVM, Android, and JavaScript support

- The Java, Kotlin, and Android rules and toolchains are removed.
- `system_demo_toolchains()` no longer defines the Java, Kotlin, Android, and dex toolchains.
- The JavaScript rules of `prelude/js` are removed, together with `worker_tool`.
- `ndk_toolchain` is removed.
- The `prelude//os:building_android_binary`, `prelude//os:maybe_building_android_binary`, and `prelude//runtime/constraints:test_runtime` constraints are removed. `prelude//os:android` stays.
- `cxx_library`, `cxx_precompiled_header`, `prebuilt_cxx_library`, `apple_binary`, `apple_library`, and `apple_test` lose `can_be_asset`.
- `cxx_library` and `cxx_precompiled_header` lose `used_by_wrap_script`.
- `cxx_library`, `cxx_precompiled_header`, `prebuilt_cxx_library`, `prebuilt_cxx_library_group`, and `rust_library` lose `include_in_android_merge_map_output`.
- `remote_file` no longer accepts `mvn:` URLs. The `[http] maven_repo` and `[http] maven_repo_override` settings are removed with them.
- `yak audit classpath` is removed. It returned an error for every input.
- Query attributes no longer have the `classpath()` function.
- `yak install` loses the Android flags (`--run`, `--emulator`, `--device`, `--serial` or `--udid`, `--all-devices`, `--activity`, `--intent-uri`, `--wait-for-debugger`, `--uninstall`, `--keep`, and their short forms). The arguments after `--` still go to the installer.
- `prelude/debugging` is removed. It held the BXL half of Meta's internal debugger.
- `prelude/graphql` is removed. No rule in this repository created its providers.

`zip_file` builds its archive with `prelude//zip_file/tools:create_zip`, a Python script, in place of a Java tool:

- `zip_file` needs a Python bootstrap toolchain in place of a Java runtime.
- `zip_file_toolchain` takes the tool as `create_zip`, which defaults to `prelude//zip_file/tools:create_zip`.
- Entries are sorted by name. The Java tool wrote the entries of each archive in `zip_srcs` together.
- Entries copied from `zip_srcs` keep their permissions. The Java tool dropped them.
- The directory entries that `zip_file` adds for the parent directories of `srcs` files have the permissions 0755.
- `entries_to_exclude` patterns use the syntax of Python's `re` module in place of Java's `java.util.regex`.

### Removed Buck1 compatibility

Commands, flags, and build file functions:

- `yak build` loses the hidden `--deep` flag, which did nothing.
- `yak test` loses the hidden `--deep` and `--xml` flags, which did nothing.
- The JSON that `yak run --command-args-file` writes loses the `is_fix_script` and `print_command` fields.
- Test executors no longer receive the `--buck-test-info ignored` arguments.
- The `--output-attributes` flag is removed. It returned an error that named `--output-attribute`.
- The `labels()` query function is removed. It returned an error for every input.
- `yak targets --target-hash-function` accepts `fast` and `strong`. The `sha1`, `sha256`, and `murmur_hash3` values, which chose one of the two, are removed.
- Target labels and patterns lose the `#flavor` suffix. `//foo:bar#headers` fails as an invalid target name. `//foo:bar[headers]` names the subtarget that `#headers` mapped to.
- `repository_name()` is removed. `get_cell_name()` returns the same name without the leading `@`.
- Entries of `[buildfile] package_includes` take the form `<package path>=><import path>`. The `::` list of symbols and its `alias=symbol` renames are removed. `implicit_package_symbol()` looks up each symbol by its name in the imported file.

The build report:

- The build report loses the `failures` and `truncated` fields.
- Each entry of `results` loses the `success`, `outputs`, `other_outputs`, and `configured_graph_size` fields that merged its configurations. Each entry of `configured` keeps them.
- The `fill-out-failures` value of `--build-report-options` is removed.
- The `[build_report] print_unconfigured_section` setting is removed.

The prelude:

- `sh_test` loses `list_args`, `list_env`, `run_args`, and `run_env`, which failed analysis when set.
- `cxx_test` loses `framework`, which did nothing.
- Rules lose `default_platform` and `link_deps_query_whole`, which no rule read.
- Rules lose `defaults`. A `static`, `static_pic`, or `shared` value in it set the link style of C++ and Apple targets that left `link_style` unset.
- Genrules no longer set the `GEN_DIR` environment variable, whose value was `GEN_DIR_DEPRECATED`.
- On Windows, genrules no longer rewrite `$OUT` and `${OUT}` in a command to `%OUT%`. The rewrite also covered `SRCDIR`, `SRCS`, `TMP`, and the variables that rules added. Commands that run on Windows use the `%OUT%` form.
- Scripts that `sh_binary` writes no longer set `YAK_SH_BINARY_VERSION_UNSTABLE`.
- `@prelude//platforms/apple:base.bzl` loses `apple_generated_platforms`.
- The `legacy_toolchain`, `external_test_runner`, and `python_test_runner` rules are removed. They failed analysis for every target.
- `configured_alias` loses the `actual` attribute, which query output showed and analysis ignored. The `configured_alias` macro still takes `actual`.
- `versioned_alias` is removed.
- `prebuilt_cxx_library` loses its `versioned_*` attributes.
- `python_library` and `python_test` lose `versioned_srcs` and `versioned_resources`.
- `attrs.versioned()` is removed. A `select()` on constraints replaces the versioned attributes.

### Removed commands and flags

- `buck2 rage` is removed. It uploaded diagnostics to Meta's internal services.
- `buck2 explain` is removed. It printed nothing outside Meta's build.
- `buck2 debug upload-re-logs` is removed. It uploaded Remote Execution logs to Manifold.
- `buck2 debug persist-event-logs` is removed. It uploaded event logs to Manifold.
- `buck2 docs agent` is removed. It printed Meta's schema for `--agent-context`.
- The global `--agent-context` flag and the `CODING_AGENT_METADATA` environment variable are removed. They tagged invocations for Meta's analytics.
- `buck2 query --output-format html` is removed. It uploaded the page to Manifold.

### Removed analytics

- The Scribe, Scuba, and Manifold clients are deleted, together with every code path that sent events, logs, or metrics to them.
- The client no longer adds an `id` entry to `--client-metadata` for analytics.

### Event log

- `ActionExecutionEnd` no longer records the host name of a local action.
- The instant events `RageResult`, `IoProviderInfo`, `PersistEventLogSubprocess`, `ActionDigestTrace`, and `ReLogStreamAvailable` are removed. On an event log from an earlier version that holds one of them, `buck2 log replay` and `buck2 log whatup` fail with ``Missing `data` in `Instant` ``.

### Removed configuration

- `buck2 init` no longer writes the `none` cell or the `fbcode`, `fbsource`, `fbcode_macros`, `buck`, and `ovr_config` cell aliases. A project that loads through those names needs its own `[cell_aliases]` entries.
- The `[buck2_health_check]` section is removed.
- The `[scuba] defaults` key is removed.
- The `[agent_context] enforced_clients` key is removed.
- The `[buck2]` keys for Eden are removed (`allow_eden_io`, `detect_eden_restart`, `use_eden_thrift_read`).
- The `[buck2]` keys for event log upload are removed (`log_use_manifold`, `event_log_buffer_size`, `event_log_message_batch_size`, `event_log_retry_attempts`, `event_log_retry_backoff_duration_ms`).
- The `log_use_manifold` setting in `[log_download]` is removed. `log_url` still selects a server for `buck2 log` downloads.
- The `[buck2]` keys `agent_hostname_fail_v2_context`, `agent_hostname_fail_v2_glob`, and `allow_daemon_start_unsandboxed_via_wrapper` are removed.
- The `[http] proxy_env_allowlist` key is removed. Only Meta's internal HTTP client read it. `HTTPS_PROXY`, `HTTP_PROXY`, and `NO_PROXY` work as before.
- The environment variables for Eden (`BUCK2_DISABLE_EDEN_HEALTH_CHECK`, `BUCK2_EDEN_SEMAPHORE`, `BUCK2_ENABLE_EDEN_THRIFT_READ`) are removed.
- The environment variables for log upload (`BUCK2_SCRIBE_CATEGORY`, `BUCK2_TEST_MANIFOLD_TTL_S`, `BUCK2_TEST_MANIFOLD_CHUNK_BYTES`, `BUCK2_TEST_BLOCK_ON_UPLOAD`, `BUCK2_TEST_DISABLE_LOG_UPLOAD`) are removed.
- The CI metadata variables that tagged events for analytics (`SANDCASTLE`, `SANDCASTLE_ALIAS`, `SANDCASTLE_ID`, `SANDCASTLE_INSTANCE_ID`, `SANDCASTLE_JOB_INFO`, `SANDCASTLE_SCHEDULE_TYPE`, `SANDCASTLE_TYPE`, `SCHEDULE_TYPE`, `SKYCASTLE_WORKFLOW_ALIAS`, `SKYCASTLE_WORKFLOW_RUN_ID`) are no longer read.
- `BUCK2_ACTION_DIGEST_TRACE_LG_SAMPLE_RATE`, `BUCK2_DICE_DUMP_ON_PANIC`, `BUCK2_DUMP_FBS`, `BUCK2_IGNORE_VERSION_EXTRACTION_FAILURE`, and `BUCK2_TEST_DAEMON_ORIGINATING_CGROUP` are removed.
- The `BUCK2_TEST_EXECUTOR_USE_TCP` environment variable replaces `BUCK2_TEST_TPX_USE_TCP`.
- The daemon no longer reads `~/.buckconfig.d/experiments_from_buck_start`, in which Meta's wrapper listed Gatekeeper experiments. Commands no longer log a `TagEvent` of its `[experiments]` keys.

### Remote Execution

The Bazel Remote Execution API has no field for gang workers, action dependencies, or a custom image, so the executor dropped them before each request. Their parameters are removed:

- `CommandExecutorConfig` no longer accepts `remote_execution_dependencies`, `remote_execution_gang_workers`, or `remote_execution_dynamic_image`.
- `ctx.actions.run` no longer accepts `remote_execution_dependencies`, `re_gang_workers`, or `remote_execution_dynamic_image`.
- `genrule` and the rules built on `genrule_attributes()` lose the `remote_execution_dependencies` attribute.
- A test whose `remote_execution` properties set `dependencies`, `gang_workers`, or `remote_execution_dynamic_image` fails attribute coercion.
- `get_re_executors_from_props` in `@prelude//tests:re_utils.bzl` loses its `dynamic_image_override` parameter.
- `remote_test_execution_toolchain` loses `default_run_as_bundle`.
- The event log no longer reports the `queue_acquiring_dependencies` Remote Execution stage.

### Documentation links

- Help text and error messages link to https://rdeusser.github.io/buck2/ in place of buck2.build.

### Prelude

- C++ header units compile with `-DPRELUDE_CPP_HEADER_UNIT=1` in place of `-DFACEBOOK_CPP_HEADER_UNIT=1`.
- Late-stamped build info goes into an ELF section named `build_info` in place of `fb_build_info`.
- The `tests.disable_re_tests` yakconfig key replaces `fbcode.disable_re_tests`.
- A Remote Execution test without a `use_case` no longer falls back to the `tpx-default` use case.
- `BuckconfigBackedModifier` no longer has an `oncall` field.
- Test rules no longer add the labels that only Meta's Tpx test runner read (the `tpx:*` labels of `apple_test`, and the `run_as_bundle` label).
- `go_test` no longer sets `TPX_LIST_TESTS_COMMAND`.
- `python_needed_coverage_test` no longer sets `TEST_PILOT`.
- `buck2 run` on a `go_test` without `env` runs the test binary directly. It ran through `inject_test_env.py` before.
- The `TestListingInfo` provider and the `prelude//go/tools:list_tests` target are removed.
- The `@prelude//apple:apple_test_device_types.bzl` module is removed.

Rule attributes and toolchain fields that only Meta's build used are removed:

- Apple rules lose `bundle_telemetry_logger`, `entitlements_verification_check_enabled`, `_fast_adhoc_signing_probe_enabled`, `_meta_apple_library_validation_enabled`, and `_sanitizer_compatibility`. `apple_test` loses `test_device_type`. `AppleToolchainInfo` loses `bundle_telemetry_logger`.
- C++ rules lose `use_fbcc_rust_wrapper`.
- `cxx_toolchain` loses `_dumpbin_toolchain_path`.
- `CxxToolchainInfo` loses `compiler_with_wrapper`.
- `http_archive` and `http_file` lose `vpnless_urls`.
- `remote_file` loses `vpnless_url`.
- Python rules lose `use_rust_make_par`, `use_rust_make_par_optimizations`, and `opt_by_default_enabled`. `PythonToolchainInfo` loses `gen_lpar_bootstrap`, `make_py_package_live`, and `manifest_module_entries`.
