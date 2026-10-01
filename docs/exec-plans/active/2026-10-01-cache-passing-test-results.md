# Cache passing test results

Follows [the plan contract](../../PLANS.md).

## Purpose

`yak test` reports a test as passed without running it when the test passed before with the same inputs, as `go test` does. The inputs are the test command, its environment, and the contents of every file the command reads: the test binary, its resources, and its declared test data. A daemon restart keeps the results. With a remote cache configured, a pass recorded on one machine counts on every machine, so a CI job runs only the tests whose inputs changed. `yak test --no-test-cache` runs every selected test, as `go test -count=1` does.

The repository owner's requirements (2026-10-01): the cache works like Go's test cache, it has a flag that runs every test, and remote execution shares results across machines.

To see it working, run `yak test //...` twice in a Cargo workspace. The second run reports every test as a cached pass and runs no test command. After an edit to one crate, a third run runs the tests of that crate and of the crates that depend on it.

## Progress

- [x] Milestone 1: a local cache of passing results for `rust_test` and `go_test`, which survives daemon restarts.
- [x] Milestone 2: `yak test --no-test-cache`, the cached-pass display, and documentation.
- [x] Milestone 3: action digests of tests that do not depend on the path of the checkout.
- [ ] Milestone 4: lookups in and uploads to a remote action cache.
- [ ] Milestone 5: validation against Roost and the GitHub CLI.

## Surprises & Discoveries

- The orchestrator runs a test command in `prepare_and_execute_inner` (`app/yak_test/src/orchestrator.rs`). For `TestStage::Testing` it asks `CommandExecutor::action_cache` first when the test's `ExternalRunnerTestInfo` sets `supports_test_execution_caching` and the request leaves `disable_test_execution_caching` unset. It never uploads a test result. Test listing (`TestStage::Listing`) both looks up and uploads.
- Without remote execution, the action cache checker is `NoOpCommandOptionalExecutor` (`app/yak_server/src/daemon/common.rs`), so no test result is ever reused locally.
- `supports_test_execution_caching` is an attribute of `cxx_test`, the Python test rules, and `sh_test`, and defaults to `False`. `rust_test` and `go_test` lack it.
- A local test command starts from an empty environment plus the values of an allowlist (`EnvironmentInheritance::test_allowlist` in `app/yak_execute/src/execute/environment_inheritance.rs`), such as `PATH`, `HOME`, `USER`, and `TMPDIR`. The allowlisted values are not part of the action digest.
- `yak_test_runner` passes `false` for `disable_test_execution_caching` on every test (`app/yak_test_runner/src/runner.rs`) and derives the status from the exit code alone.
- The daemon persists its local action cache for builds in a SQLite database (`app/yak_execute_impl/src/sqlite/dep_file_state_db.rs`, `[yak] sqlite_dep_file_state`), which is the model for a persisted store.
- The action digest of a test that a `cargo` cell declares depends on the path of the checkout. Its argv names the test binary by absolute path, and `run_from_manifest_dir` sets `CARGO_MANIFEST_DIR` to an absolute path at run time. Two copies of `tests/core/test/test_test_result_cache_data/workspace` in different directories built byte-identical test binaries (SHA-256 `d7e9e393cea10a60…`) and stored passes under different keys. `YAK_BUILD_ID` and `YAK_DAEMON_UUID` are added to the environment after the digest, so they do not change it. The argv of a `go_test` names its binary by absolute path as well, and two copies of a one-package module built byte-identical test binaries (SHA-256 `1d8a219363f04925…`) and stored passes under different keys. A remote pass could therefore count only on machines that use the same checkout path, until milestone 3 replaced the root with a placeholder in the digest.

## Decision Log

- 2026-10-01: Only passing results are cached. A failing or timed-out test runs again, as with Go.
- 2026-10-01: The local key is the test command's action digest and the values of the inherited environment allowlist. The action digest covers the command line, the action's environment, and the digest of every input file, so a change to the binary, a resource, or declared test data changes the key. The allowlisted values come from the daemon's environment and would otherwise let a pass under one `PATH` count under another.
- 2026-10-01: The action digest covers absolute paths with a placeholder for the project root, so a pass counts on every machine. The alternative of rendering the paths relative to the working directory would change what the test sees, such as a relative `CARGO_MANIFEST_DIR` where Cargo gives an absolute one. The normalization happens where the digest is computed, so the executors and the commands that `yak log what-ran` prints keep the real paths.
- 2026-10-01: The remote key is the action digest alone, as for the remote test caching that the orchestrator already does. Including the inherited environment would keep two machines with different `HOME` values from sharing any result. A test that depends on an inherited value can pass from another machine's result, and `--no-test-cache` runs it.
- 2026-10-01: `rust_test` and `go_test` take `supports_test_execution_caching`, which defaults to `False`, and the `cargo` and `go` cells set it to `True`. A test that a cell declares runs in a copy of its declared files. A hand-written `rust_test` runs from the project root unless it sets `run_from_manifest_dir`, so it can read files it does not declare.
- 2026-10-01: `--no-test-cache` skips lookups and still records new passes.
- 2026-10-01: The store is a directory of entries under `yak-out/<isolation dir>/cache/test_results` in place of a SQLite table. Each entry is a directory of the two streams, which appears through one rename, so concurrent writers need no lock, and the daemon's startup cleanup keeps the directory through `valid_cache_dirs`.
- 2026-10-01: A cached pass is a `CommandExecutionKind::LocalActionCache` with a `LocalCacheHit` command in events, and its executor stage is a `CacheHit` of the new `CACHE_TYPE_LOCAL_ACTION_CACHE`, so `yak log what-ran` lists it as `local_cache`. The test runners mark the `TestResult` as `cached` from the execution kind, which also covers remote cache hits.

## Outcomes & Retrospective

Nothing yet.

## Context and Orientation

- `app/yak_test/src/orchestrator.rs`: `YakTestOrchestrator::execute2` serves the test runner's requests, `prepare_and_execute` decides whether a stage runs on DICE, and `prepare_and_execute_inner` prepares the action and runs it, with the cache lookup for listing and testing.
- `app/yak_execute/src/execute/command_executor.rs`: `CommandExecutor::action_cache` and `cache_upload`.
- `app/yak_execute_impl/src/executors/action_cache.rs`: `ActionCacheChecker`, the remote action cache lookup.
- `app/yak_execute_impl/src/sqlite/`: the SQLite stores that the daemon keeps under its state directory.
- `app/yak_test_runner/src/runner.rs`: the test runner, which turns an `ExecutionResult2` into a `TestResult`.
- `app/yak_client_ctx/src/subscribers/superconsole/test.rs` and the simple console print each test's result.
- `prelude/rust/rust_binary.bzl` and `prelude/go/go_test.bzl` build each test's `ExternalRunnerTestInfo`.

## Plan of Work

1. A local store of passing results (`app/yak_test/src/test_result_cache.rs`) under `yak-out/<isolation dir>/cache/test_results`. Each entry holds stdout and stderr, each up to 1 MiB. A test whose streams exceed the limit, or that declares outputs, is not stored. `prepare_and_execute_inner` computes the key after `prepare_action`, returns a stored pass as a `LocalActionCache` execution, and stores the result of a passing local run. `rust_test` and `go_test` take `supports_test_execution_caching`, and the `cargo` and `go` cells set it. Integration tests (`tests/core/test/test_test_result_cache.py`, `tests/core/external_cells/test_go.py`) check a cached second run, a cached run after a daemon restart, a rerun after an edit to declared test data, and a rerun of a failing test.
2. `yak test --no-test-cache` sets `no_test_cache` in the test session options, which turns off lookups for every test of the command. A cached pass prints as `✓ Pass (cached)`, and the summary counts cached passes with the passes. Documentation: `website/docs/rule_authors/test_execution.md`, the Cargo and Go pages, and `CHANGELOG.md`.
3. Path-independent digests. A test whose `ExternalRunnerTestInfo` sets `use_project_relative_paths = False` gets absolute paths, and `CommandExecutionRequest::with_absolute_paths_under` records the project root. `prepare_action` digests the arguments and environment with `PROJECT_ROOT_PLACEHOLDER` (`${YAK_PROJECT_ROOT}`, `app/yak_execute/src/execute/project_root_placeholder.rs`) in place of the root, where the root starts a path. The executors, events, and `yak log what-ran` keep the real paths. `test_the_key_does_not_depend_on_the_checkout_path` runs a test in two copies of a project and compares the stored keys.
4. The remote cache. With a remote cache configured, a passing local run uploads its result under the action digest, as the listing stage does, unless the build skips cache writes. The existing lookup serves remote hits. Validation needs a remote cache, such as `bazel-remote`, in an integration test or a recorded manual run.
5. Validation. In Roost and the GitHub CLI, record the time of a first and a second `yak test //...`, and of a run after an edit to one package.

## Validation and Acceptance

- `python3 test.py` passes.
- Integration tests under `tests/core/test/` show a cached second run, a rerun after an input change, a rerun of a failing test, and a run of every test under `--no-test-cache`.
- In Roost, a second `yak test //...` runs no test command and reports 8 cached passes.

## Idempotence and Recovery

Deleting `yak-out/<isolation dir>/cache/test_results`, or `yak clean`, empties the cache, and the next run runs every test again. A change to the entry format needs a new directory name.
