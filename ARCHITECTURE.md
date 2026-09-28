# Architecture

This repository holds Buck2, a build system, together with the Starlark interpreter, the incremental computation engine, the rule library, and the utility crates it is built from.
A user declares targets in build files (`BUCK` by default) in Starlark. Buck2 evaluates those files, configures each target for a platform, runs each rule's analysis to produce actions, and runs the actions locally or on a Remote Execution service.
Buck2 keeps its results in an incremental computation graph, so a later command recomputes only what its changed inputs affect.

The repository is a fork of `facebook/buck2`. [Planned changes](#planned-changes) lists what the fork intends to change.
`website/docs/concepts/architecture.md` describes the build phases for users. This document maps them to the code.

## Bird's-eye view

`buck2` runs as two processes built from one binary.
The client parses the command line, connects to a daemon or starts one, and renders the events the daemon streams back.
The daemon (`buckd`) holds all state between commands and does all build work. It is the hidden `buck2 daemon` subcommand.
`--no-buckd` runs the daemon inside the client process.

Build state in the daemon lives in DICE, an incremental computation engine.
A DICE `Key` computes its value from other keys, and DICE records each read as a dependency edge.
At the start of each command the file watcher reports changed files to DICE, which recomputes only the keys that depend on them.

A build moves through four stages, each a family of DICE keys:

1. Loading. `InterpreterResultsKey` evaluates one package's build file and returns the unconfigured `TargetNode`s it declares. `.bzl` files load through `EvalImportKey`.
2. Configuration. `ConfiguredTargetNodeKey` applies a target platform, modifiers, and transitions to a `TargetNode` and returns a `ConfiguredTargetNode`.
3. Analysis. `AnalysisKey` runs the rule's `impl` function on a configured node and returns an `AnalysisResult`, which holds the target's providers and the actions it registered.
4. Execution. `BuildKey` runs one action through a command executor and records its outputs. The deferred materializer writes outputs that exist only remotely to `buck-out` when a local action or the user needs them.

## Code map

Paths are relative to the repository root. Rust crates under `app/` share the `buck2_` prefix.

### Entry point and process model

- `app/buck2` is the `buck2` binary. `main` in `bin/buck2.rs` initializes every late binding before any thread starts, and `src/lib.rs` defines the top-level command line and dispatches each subcommand.
- `app/buck2_client_ctx` is the client runtime. It connects to or starts the daemon (`connect_buckd`, `BootstrapBuckdClient`), runs commands (`StreamingCommand`), and fans daemon events out to each `EventSubscriber`, such as the superconsole and the event log writer.
- `app/buck2_client` implements most client commands (`build`, `test`, `targets`, `kill`, `status`, and others) in `src/commands/`.
- `app/buck2_cmd_*_client` crates implement the remaining client commands (`audit`, `completion`, `debug`, `docs`, `log`, `starlark`).
- `app/buck2_daemon` is the `buck2 daemon` process. It daemonizes, writes `buckd.pid` and `buckd.info` in the daemon directory (`~/.buck/buckd/<project root>/<isolation dir>/`), and starts the server. `app/buck2_daemon/daemon_lifecycle.md` describes the startup, connection, and shutdown protocol.
- `app/buck2_cli_proto` defines the client-daemon protocol. `daemon.proto` declares `service DaemonApi`.
- `app/buck2_server` implements that service in `BuckdServer`, holds the daemon's state, and gives each request a `ServerCommandContext`.
- `app/buck2_server_ctx` declares the interfaces that server commands implement, and `app/buck2_server_commands` and the `app/buck2_cmd_*_server` crates implement them (`build`, `install`, `audit`, `docs`, `query`, `starlark`, `targets`).
- `app/buck2_concurrency` decides whether a command can run while other commands hold the daemon's DICE state (`ConcurrencyHandler`).

The isolation directory (`--isolation-dir`, default `v2`) names both the daemon directory and the output directory `buck-out/<isolation dir>/`.
Two invocations in the same project share a daemon if and only if their isolation directories match.
A client restarts the daemon when the daemon's `DaemonConstraints` do not satisfy the client's request, for example when the binary version or the daemon startup configuration differs.

### Incremental computation

- `dice/dice` is DICE. A computation implements `Key` (`compute`, `equality_behavior`) and reads other keys through `DiceComputations`. Changes enter through `DiceTransactionUpdater` (`changed`, `changed_to`, `commit`). `dice/dice/docs/index.md` documents the engine.
- `dice/dice_core` tracks versions and revisions to decide when a computed value can be reused. `dice/docs/incrementality.md` specifies that model.
- `dice/dice_futures` and `dice/dice_error` provide cancellation and error types.
- `pagable`, `pagable_derive`, and `pagable_storage` serialize values so the daemon can page DICE state out to SQLite or sled and back in. DICE keys implement `Pagable`. Paging is off unless configuration enables it (`HydrationConfig` in `app/buck2_common/src/init.rs`).

### Core types and utilities

- `app/buck2_fs` defines filesystem paths that know nothing of cells or projects (`ForwardRelativePath`, `AbsNormPath`, `FileName`).
- `app/buck2_core` defines cells (`CellName`, `CellPath`, `CellResolver`), project paths (`ProjectRelativePath`), packages and targets (`PackageLabel`, `TargetLabel`, `ConfiguredTargetLabel`), provider labels (`ProvidersLabel`), configurations (`ConfigurationData`), and target patterns (`ParsedPattern`). It does not depend on the `dice` crate.
- `app/buck2_common` reads files (`DiceFileComputations`, `FileOps`), buckconfig (`LegacyBuckConfig`, `HasLegacyConfigs`), cells, package listings, and build file names (`buildfiles.rs`) through DICE.
- `app/buck2_error` defines the error type used across the workspace (`buck2_error::Error`, `buck2_error::Result`), error tags (`ErrorTag`, generated from `app/buck2_data/error.proto`), and the `buck2_error!` and `internal_error!` macros.
- `app/buck2_env` defines the `buck2_env!` macro (which reads an environment variable and registers it for `buck2 help-env`) and the soft error macros.
- `app/buck2_util`, `app/buck2_hash` (`BuckMutMap`), `app/buck2_directory` (directory trees and their digests), and `app/buck2_http` (HTTP client) hold shared utilities.

### Loading and Starlark

- `starlark-rust/` is the Starlark interpreter (`starlark`), its parser and AST (`starlark_syntax`), its ordered maps (`starlark_map`), the `#[starlark_module]` and `#[starlark_value]` macros (`starlark_derive`), and an LSP server (`starlark_lsp`). It depends on no Buck2 crate.
- `app/buck2_interpreter` holds the Starlark plumbing that build files, `.bzl` files, and BXL share (module paths, loaded modules, and the slots that downstream crates fill with globals).
- `app/buck2_interpreter_for_build` evaluates `BUCK`, `.bzl`, and `PACKAGE` files and defines the build-file globals (`rule`, `attrs`, `select`, `read_config`, and others). `InterpreterResultsKey` and `EvalImportKey` live in `src/interpreter/calculation.rs`. `AttributeSpecExt::parse_params` checks each attribute value against the rule's `AttributeSpec` and converts it to a `CoercedAttr` through `AttrTypeCoerce` (`src/attrs/coerce.rs` and `src/attrs/coerce/`).
- `app/buck2_node` defines the target graph (`TargetNode`, `ConfiguredTargetNode`, `CoercedAttr`, `ConfiguredAttr`) and the traits through which other crates request nodes.
- `app/buck2_external_cells` serves cells whose files come from outside the repository, from the binary (`bundled`) or from Git (`git`). `app/buck2_external_cells_bundled` embeds `prelude/` in the binary at compile time (`build.rs` under Cargo, a generated `prelude/contents.rs` under Buck).

### Configuration

- `app/buck2_configured` computes configured target nodes (`ConfiguredTargetNodeKey`), target platforms, and execution platform resolution (`ExecutionPlatformResolutionKey`).
- `app/buck2_transition` implements the `transition()` global and applies transitions (`TransitionKey`).
- `app/buck2_cfg_constructor` implements `set_cfg_constructor()` and modifier evaluation. Only the root `PACKAGE` file can register a constructor.

### Analysis and queries

- `app/buck2_analysis` runs a rule's `impl` function (`AnalysisKey`, `run_analysis`) and resolves configured attributes into `ctx.attrs` values.
- `app/buck2_build_api` defines the analysis and build API. It holds `AnalysisResult`, `AnalysisRegistry` and `ActionsRegistry`, the Starlark `AnalysisContext`, providers (`ProviderCollection`, built-in providers declared with `#[internal_provider]`), Starlark artifacts, and the build entry points (`build_configured_label`, `ensure_artifact_group`, `BuildKey`).
- `app/buck2_artifact` defines artifact and action identities (`SourceArtifact`, `BuildArtifact`, `ActionKey`).
- `app/buck2_anon_target` implements anonymous targets and promise artifacts.
- `app/buck2_bxl` implements BXL, Starlark scripts that inspect and build the graph (`BxlKey`).
- `app/buck2_query_parser` parses query expressions, `app/buck2_query` evaluates them over any `QueryEnvironment`, and `app/buck2_query_impls` implements the uquery, cquery, and aquery environments.
- `app/buck2_validation` implements validation actions.

### Execution and materialization

- `app/buck2_action_impl` implements the Starlark actions (`run`, `copy`, `write`, `write_json`, `download_file`, dynamic actions, and others). `RunAction` implements `ctx.actions.run`.
- `app/buck2_execute` declares the execution interfaces. `PreparedCommandExecutor` runs a command, `Materializer` puts outputs on disk, and `RemoteExecutionClient` wraps the Remote Execution connection.
- `app/buck2_execute_impl` implements them. `src/executors/` holds `LocalExecutor`, `ReExecutor`, `HybridExecutor`, the action cache checkers, and persistent workers. `src/materializers/deferred.rs` holds `DeferredMaterializer`, the only materializer. `src/sqlite/` keeps materializer, dep-file, and incremental state on disk.
- `app/buck2_execute_local` spawns local processes and streams their output. On Unix the daemon spawns them through a forkserver process (`app/buck2_forkserver`).
- `remote_execution/re_grpc` (package `remote_execution`) is the client for the Bazel Remote Execution API v2. `app/buck2_re_configuration` reads its settings from the `[buck2_re_client]` buckconfig section.
- `app/buck2_resource_control` limits the memory of local actions with Linux cgroup v2. `host_sharing` limits how many local commands and tests run at once.
- `app/buck2_file_watcher` reports file changes to DICE. The `buck2.file_watcher` buckconfig key selects `notify` (the default in this repository), `watchman`, or `fs_hash_crawler`.

An execution platform's `CommandExecutorConfig` selects local, remote, or hybrid execution for each action.
Without an execution platform, this repository's build runs every action locally (`get_default_executor_config` in `app/buck2_server/src/daemon/common.rs`).

### Tests

- `app/buck2_test` is the daemon side of `buck2 test` (`BuckTestOrchestrator`, `TestExecutionKey`). It launches a test executor process and serves the orchestrator API to it over gRPC.
- `app/buck2_test_api` and `app/buck2_test_proto` define the protocol between Buck2 and a test executor (`TestExecutor`, `TestOrchestrator`).
- `app/buck2_test_runner` is the built-in test executor, which runs as `buck2 internal-test-runner`. Buck2 uses it unless `[test] v2_test_executor` names another executable.

### Events and logs

- `app/buck2_data` defines the event schema. `data.proto` declares `BuckEvent`.
- `app/buck2_events` creates and dispatches events inside a process (`EventDispatcher`, `EventSink`).
- `app/buck2_event_log` writes and reads event logs. Each command writes `buck-out/<isolation dir>/log/<timestamp>_<command>_<trace id>_events.pb.zst`.
- `app/buck2_cmd_log_client` implements `buck2 log`, which reads those files (`what-ran`, `what-failed`, `critical-path`, `replay`, and others).
- `app/buck2_event_observer` aggregates events into the state the consoles render. `superconsole/` is the terminal UI library. `Cargo.toml` excludes it from the workspace and uses it as a path dependency.
- `app/buck2_critical_path`, `app/buck2_build_signals`, and `app/buck2_build_signals_impl` compute the critical path of a build.

### Rules and libraries

- `prelude/` holds the Starlark rules and toolchains for every supported language (`prelude/cxx`, `prelude/rust`, `prelude/python`, and others). The binary embeds it as the `prelude` cell, and `buck2 init` configures new projects to use that copy.
- `gazebo/` holds small utility crates. `dupe` defines `Dupe`, a clone that is constant time and allocation-free.
- `allocative/` measures memory use per type (`Allocative`).
- `shed/` holds generic data structures that know nothing of Buck2 (`lock_free_hashtable`, `static_interner`, `provider`, and others).
- `games/` holds terminal games that the superconsole can show during a build.

### Build and tooling

- `Cargo.toml` defines the Cargo workspace. CI builds and tests the repository with Cargo.
- `.buckconfig`, the `BUCK` files, `build_defs/`, `toolchains/`, `third-party/`, and `bootstrap/` build the same crates with Buck2 (see [Two build definitions](#two-build-definitions)).
- `test.py` runs clippy, rustdoc, and the unit and doc tests. CI runs it after building the binary.
- `integrations/rust-project` generates `rust-project.json` for rust-analyzer from Buck targets.
- `tools/starlark_fmt` formats Starlark and build files.
- `explorer/` is an Electron app for exploring the graph. It is not part of the Cargo workspace.
- `examples/` holds example projects, which the manually triggered `.github/workflows/build-and-examples.yml` builds.
- `tests/` holds the pytest integration tests, which `.github/workflows/integration-tests.yml` runs on Linux (see `tests/README.md`).
- `website/` holds the user documentation site, which Docusaurus builds. Its pages live in `website/docs/`, where `website/gen_docs.py` also writes the API, rule, and command reference pages that a built `buck2` generates. `.github/workflows/upload_buck2.yml` deploys the built site to the `gh-pages` branch on every push to `main`.
- `docs/` holds contributor documentation (`docs/developers/`), the execution plan contract (`docs/PLANS.md`), and the plans and tech-debt tracker (`docs/exec-plans/`). The site does not include it.

## Invariants and boundaries

### Late binding

A lower crate declares a `LateBinding` static (`app/buck2_util/src/late_binding.rs`) and calls through it, and a downstream crate supplies the implementation at startup.
This keeps heavy crates such as `buck2_bxl` out of most crates' dependency graphs, which shortens rebuilds.
Each implementing crate exposes `init_late_bindings()`, and `main` in `app/buck2/bin/buck2.rs` calls all of them before any thread starts.
`LateBinding::get` returns an internal error that names a binding nobody initialized.
Test crates that need bindings from several crates (such as `app/buck2_build_api_tests`) initialize them with `#[ctor]`.

The same goal produces pairs of an interface crate and an implementation crate (`buck2_execute` and `buck2_execute_impl`, `buck2_build_signals` and `buck2_build_signals_impl`, `buck2_query` and `buck2_query_impls`, `buck2_server_ctx` and `buck2_server_commands`).
Code that needs the behavior depends on the interface crate.

### Crate dependency rules

`app_dep_graph_rules/rules.bzl` records these rules:

- Only `app/buck2` depends on `buck2_anon_target`, `buck2_bxl`, `buck2_cmd_audit_server`, `buck2_cmd_query_server`, `buck2_cmd_targets_server`, and `buck2_query_impls`. They reach the rest of the program through late bindings.
- Only `app/buck2` depends on `buck2_cmd_debug_client` and `buck2_cmd_log_client`.
- Neither crate in each of these pairs depends on the other, directly or transitively:
  - `buck2_common` and `buck2_directory`
  - `buck2_common` and `starlark`
  - `buck2_build_api` and `buck2_execute_impl`
  - `buck2_build_api` and `buck2_interpreter_for_build`
  - `buck2_server` and `buck2_server_commands`
  - `buck2_bxl` and `buck2_configured`
- The client-only binary does not depend on the Remote Execution client. The Buck build produces that binary (`buck2_client-bin`, compiled with `--cfg client_only`), which leaves out the daemon, the server, and every late-binding implementation.

`//app_dep_graph_rules:test_buck2_dep_graph` checks the rules during analysis, so `buck2 build //app_dep_graph_rules:test_buck2_dep_graph` fails when a dependency breaks one. CI runs no Buck build, and the tech-debt tracker records that gap.

### DICE correctness

- A computation reads files, buckconfig, and other computed values only through DICE. DICE then records the dependency and invalidates the value when the input changes. Uncached helpers such as `get_interpreter_results_uncached` must not run inside a `Key::compute`, because DICE cannot see their reads.
- A key's `equality_behavior` never reports two different values as equal. A false equality leaves dependent values stale.
- A feature gate is a key in the `[buck2]` buckconfig section read through DICE. `buck2_env!` environment variables are for the client before it reaches the daemon and for test-only settings.

### Other invariants

- Code calls `ensure_materialized` on an artifact's path before reading it from disk. `Materializer` in `app/buck2_execute/src/materialize/materializer.rs` lists the full set of materializer invariants.
- A breaking change to a SQLite schema in `app/buck2_execute_impl/src/sqlite/` bumps that database's hand-maintained schema version constant (for example `MATERIALIZER_DB_SCHEMA_VERSION`).
- Each `TargetLabel` is interned once and compared by pointer.
- `Dupe` is implemented only where `clone` is constant time and allocation-free, and `dupe` always calls `clone`.

## Where input becomes typed values

- Command-line arguments become clap structs in the client crates, which send protobuf requests from `app/buck2_cli_proto` to the daemon.
- Buckconfig files become `LegacyBuckConfig`. Code reads a key through `BuckconfigKeyRef` and parses it into a typed value at the read.
- Environment variables that configure Buck2 are declared with `buck2_env!`, which parses each into a typed value. Some code in `app/` reads other variables directly with `std::env::var`.
- Build files become Starlark values, which attribute coercion (`AttrTypeCoerce`) checks against the rule's `AttributeSpec` and converts into `CoercedAttr` values on a `TargetNode`.
- Target pattern strings become `ParsedPattern` values.
- Path strings become `ForwardRelativePath`, `ProjectRelativePath`, or `AbsNormPath` through checked constructors that reject `.` and `..` components. Each of these types also has an `unchecked_new` for a string the caller has already validated.

## Cross-cutting concerns

### Errors

Fallible functions return `buck2_error::Result`.
Errors carry tags (`ErrorTag`) that classify them, and `internal_error!` marks a bug in Buck2 itself.
`docs/developers/error_handling.md` covers defining, tagging, and converting errors.

### Observability

The daemon reports progress as `BuckEvent`s, which the client renders and writes to the event log.
`buck2 log` reads the event log, and `BUCK_LOG` enables `tracing` output.
`docs/developers/debugging.md` lists the commands.

### Build flags

Two `cfg` flags change what a crate compiles:

- `#[cfg(buck_build)]` code compiles only under Buck. The macros in `build_defs/rust.bzl` set the flag. Under Cargo, `app/buck2_external_cells_bundled` embeds the prelude through `build.rs`, and `app/buck2/bin/buck2.rs` sets jemalloc as the global allocator on Linux and macOS.
- `#[cfg(client_only)]` code compiles only in the client-only binary (see [Crate dependency rules](#crate-dependency-rules)).

### Two build definitions

Each crate has a `Cargo.toml` and a `BUCK` file, and a change to its dependencies updates both.
The `BUCK` files load their Rust macros from `build_defs/rust.bzl` and `build_defs/proto.bzl`, and they name crates in this repository as `//<path>:<crate>` (for example `//app/buck2_core:buck2_core`).
They name third-party crates as `//third-party/rust:<crate>`. `reindeer buckify` generates `third-party/rust/BUCK` from `third-party/rust/Cargo.toml`, and per-crate build settings live in `third-party/rust/fixups/`.
Git ignores the generated `BUCK`. Until it exists, `third-party/rust/BUCK.missing` loads in its place and fails with the command that generates it (`[buildfile] name` in `.buckconfig`).
`toolchains/BUCK` defines the toolchains the build uses. `third-party/proto/` provides `protoc`, and `third-party/win/` provides the Windows libraries.
`.buckconfig` sets `[external_cells] prelude = bundled`, so a Buck build of this repository loads the prelude embedded in the running binary. Changes to `prelude/` take effect after a rebuild of the binary.

### Memory

Cargo builds on Linux and macOS use jemalloc, and Windows builds use mimalloc (`#[global_allocator]` in `app/buck2/bin/buck2.rs`).
Buck builds on Linux and macOS use the system allocator, because `third-party/rust/fixups/tikv-jemalloc-sys` does not build jemalloc.
`allocative` attributes memory to types, and `docs/developers/perf/memory.md` covers heap profiling.

## Planned changes

- The project will be renamed to yak, and the default build file name will change from `BUCK` to `YAK`. `docs/exec-plans/active/2026-09-28-rename-the-fork.md` tracks the work.
- `DEFAULT_BUILDFILES` in `app/buck2_common/src/buildfiles.rs` sets the default build file names (`BUCK.v2`, `BUCK`), and the `[buildfile] name` buckconfig key overrides them for a cell.
- The `com.facebook` packages of the JVM and Android toolchain will move to a package under the new name. `docs/exec-plans/tech-debt-tracker.md` lists them with the other upstream connections that remain.
