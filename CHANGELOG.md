# Buck2

## Unreleased

Removes the code, configuration, and service clients that only Meta's internal build used, and renames the tool to yak.

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
- `android_instrumentation_test` sets no default executor overrides, and its `re_caps` and `re_use_case` must name the same overrides.
- The event log no longer reports the `queue_acquiring_dependencies` Remote Execution stage.

### Documentation links

- Help text and error messages link to https://rdeusser.github.io/buck2/ in place of buck2.build.

### Prelude

- C++ header units compile with `-DPRELUDE_CPP_HEADER_UNIT=1` in place of `-DFACEBOOK_CPP_HEADER_UNIT=1`.
- Late-stamped build info goes into an ELF section named `build_info` in place of `fb_build_info`.
- The `tests.disable_re_tests` buckconfig key replaces `fbcode.disable_re_tests`.
- `versioned_alias` and versioned parameters select on `config//third-party/<project>/constraints:<version>`, which the `config` cell must define. They selected on `ovr_config//third-party/...` before.
- A Remote Execution test without a `use_case` no longer falls back to the `tpx-default` use case.
- A Kotlin target with `abi_generation_mode = "source_only"` fails during analysis. Kotlin supports `class` and `none`.
- The kapt step passes only the all-open compiler plugin, and standalone KSP passes only symbol-processing plugins. A plugin named `di.jar` went to one of them before, chosen by its `com.facebook.kotlin.di:kspActive` option.
- The Android installer enables app links only when `--enable-app-links` is passed. It enabled them by default for an allowlist of Meta's apps before.
- `BuckconfigBackedModifier` no longer has an `oncall` field.
- Test rules no longer add the labels that only Meta's Tpx test runner read (the `tpx:*` labels on `apple_test`, `android_instrumentation_test`, and the JVM test macros, and `run_as_bundle`).
- `go_test`, `java_test`, and `android_instrumentation_test` no longer set `TPX_LIST_TESTS_COMMAND`.
- `python_needed_coverage_test` no longer sets `TEST_PILOT`.
- `buck2 run` on a `go_test` without `env` runs the test binary directly. It ran through `inject_test_env.py` before.
- The `TestListingInfo` provider and the `prelude//go/tools:list_tests` target are removed.
- The `@prelude//apple:apple_test_device_types.bzl` module is removed.

Rule attributes and toolchain fields that only Meta's build used are removed:

- Apple rules lose `bundle_telemetry_logger`, `entitlements_verification_check_enabled`, `_fast_adhoc_signing_probe_enabled`, `_meta_apple_library_validation_enabled`, and `_sanitizer_compatibility`. `apple_test` loses `test_device_type`. `AppleToolchainInfo` loses `bundle_telemetry_logger`.
- `java_test_toolchain` and the Android toolchain lose `list_tests`.
- C++ rules lose `use_fbcc_rust_wrapper`.
- `cxx_toolchain` loses `_dumpbin_toolchain_path`.
- `CxxToolchainInfo` loses `compiler_with_wrapper`.
- `http_archive` and `http_file` lose `vpnless_urls`.
- `remote_file` loses `vpnless_url`.
- JavaScript rules lose `_asset_dest_path_resolver`.
- `KotlinToolchainInfo` loses `kosabi_applicability_plugin`, `kosabi_jvm_abi_gen_k2_plugin`, and `kosabi_stubs_gen_k2_plugin`.
- Python rules lose `use_rust_make_par`, `use_rust_make_par_optimizations`, and `opt_by_default_enabled`. `PythonToolchainInfo` loses `gen_lpar_bootstrap`, `make_py_package_live`, and `manifest_module_entries`.
