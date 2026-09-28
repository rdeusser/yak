# Buck2

## Unreleased

Removes the code, configuration, and service clients that only Meta's internal build used.

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
