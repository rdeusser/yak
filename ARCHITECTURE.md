# Architecture

This repository holds yak, a build system, together with the Starlark interpreter, the incremental computation engine, the rule library, and the utility crates it is built from.
A user declares targets in build files (`YAK` by default) in Starlark. yak evaluates those files, configures each target for a platform, runs each rule's analysis to produce actions, and runs the actions locally or on a Remote Execution service.
yak keeps its results in an incremental computation graph, so a later command recomputes only what its changed inputs affect.

The repository is a fork of `facebook/buck2` from commit `903bfd7a61` (2026-09-25), and it ports upstream commits one at a time instead of merging (`docs/developers/basics.md`). [Planned changes](#planned-changes) lists what the fork intends to change.
`website/docs/concepts/architecture.md` describes the build phases for users. This document maps them to the code.

## Bird's-eye view

`yak` runs as two processes built from one binary.
The client parses the command line, connects to a daemon or starts one, and renders the events the daemon streams back.
The daemon (`yakd`) holds all state between commands and does all build work. It is the hidden `yak daemon` subcommand.
`--no-yakd` runs the daemon inside the client process.

Build state in the daemon lives in DICE, an incremental computation engine.
A DICE `Key` computes its value from other keys, and DICE records each read as a dependency edge.
At the start of each command the file watcher reports changed files to DICE, which recomputes only the keys that depend on them.

A build moves through four stages, each a family of DICE keys:

1. Loading. `InterpreterResultsKey` evaluates one package's build file and returns the unconfigured `TargetNode`s it declares. `.bzl` files load through `EvalImportKey`.
2. Configuration. `ConfiguredTargetNodeKey` applies a target platform, modifiers, and transitions to a `TargetNode` and returns a `ConfiguredTargetNode`.
3. Analysis. `AnalysisKey` runs the rule's `impl` function on a configured node and returns an `AnalysisResult`, which holds the target's providers and the actions it registered.
4. Execution. `BuildKey` runs one action through a command executor and records its outputs. The deferred materializer writes outputs that exist only remotely to `yak-out` when a local action or the user needs them.

## Code map

Paths are relative to the repository root. Rust crates under `app/` share the `yak_` prefix.

### Entry point and process model

- `app/yak` is the `yak` binary. `main` in `bin/yak.rs` initializes every late binding before any thread starts, and `src/lib.rs` defines the top-level command line and dispatches each subcommand.
- `app/yak_client_ctx` is the client runtime. It connects to or starts the daemon (`connect_yakd`, `BootstrapYakdClient`), runs commands (`StreamingCommand`), and fans daemon events out to each `EventSubscriber`, such as the superconsole and the event log writer.
- `app/yak_client` implements most client commands (`build`, `test`, `targets`, `kill`, `status`, `init`, `generate`, and others) in `src/commands/`.
- `app/yak_cmd_*_client` crates implement the remaining client commands (`audit`, `completion`, `debug`, `docs`, `log`, `starlark`).
- `app/yak_daemon` is the `yak daemon` process. It daemonizes, writes `yakd.pid` and `yakd.info` in the daemon directory (`~/.yak/yakd/<project root>/<isolation dir>/`), and starts the server. `app/yak_daemon/daemon_lifecycle.md` describes the startup, connection, and shutdown protocol.
- `app/yak_cli_proto` defines the client-daemon protocol. `daemon.proto` declares `service DaemonApi`.
- `app/yak_server` implements that service in `YakdServer`, holds the daemon's state, and gives each request a `ServerCommandContext`. At the start of each command, `ServerCommandContext` runs `rustc -vV`, `go version`, and `clang --version` and adds digests of their output to the command's configuration as `tool_identity.<tool>` (`src/tool_identity.rs`). They are computed values (`ComputedConfigValue` in `app/yak_common/src/legacy_configs/args.rs`), which a `-c` of the command line overrides and which the configuration in the command's events leaves out. The toolchains of `system_toolchains()` and the `cargo` and `go` cells read them, so the actions and cells that use a tool from `PATH` depend on its version (2026-10-01, `docs/exec-plans/completed/2026-10-01-key-actions-on-their-tools.md`).
- `app/yak_server_ctx` declares the interfaces that server commands implement, and `app/yak_server_commands` and the `app/yak_cmd_*_server` crates implement them (`build`, `install`, `audit`, `docs`, `query`, `starlark`, `targets`).
- `app/yak_concurrency` decides whether a command can run while other commands hold the daemon's DICE state (`ConcurrencyHandler`).

The isolation directory (`--isolation-dir`, default `v2`) names both the daemon directory and the output directory `yak-out/<isolation dir>/`.
Two invocations in the same project share a daemon if and only if their isolation directories match.
A client restarts the daemon when the daemon's `DaemonConstraints` do not satisfy the client's request, for example when the binary version or the daemon startup configuration differs.

### Incremental computation

- `dice/dice` is DICE. A computation implements `Key` (`compute`, `equality_behavior`) and reads other keys through `DiceComputations`. Changes enter through `DiceTransactionUpdater` (`changed`, `changed_to`, `commit`). `dice/dice/docs/index.md` documents the engine.
- `dice/dice_core` tracks versions and revisions to decide when a computed value can be reused. `dice/docs/incrementality.md` specifies that model.
- `dice/dice_futures` and `dice/dice_error` provide cancellation and error types.
- `pagable`, `pagable_derive`, and `pagable_storage` serialize values so the daemon can page DICE state out to SQLite or sled and back in. DICE keys implement `Pagable`. Paging is off unless configuration enables it (`HydrationConfig` in `app/yak_common/src/init.rs`).

### Core types and utilities

- `app/yak_fs` defines filesystem paths that know nothing of cells or projects (`ForwardRelativePath`, `AbsNormPath`, `FileName`).
- `app/yak_core` defines cells (`CellName`, `CellPath`, `CellResolver`), project paths (`ProjectRelativePath`), packages and targets (`PackageLabel`, `TargetLabel`, `ConfiguredTargetLabel`), provider labels (`ProvidersLabel`), configurations (`ConfigurationData`), and target patterns (`ParsedPattern`). It does not depend on the `dice` crate.
- `app/yak_common` reads files (`DiceFileComputations`, `FileOps`), yakconfig (`LegacyYakConfig`, `HasLegacyConfigs`), cells, package listings, and build file names (`buildfiles.rs`) through DICE.
- `app/yak_error` defines the error type used across the workspace (`yak_error::Error`, `yak_error::Result`), error tags (`ErrorTag`, generated from `app/yak_data/error.proto`), and the `yak_error!` and `internal_error!` macros.
- `app/yak_env` defines the `yak_env!` macro (which reads an environment variable and registers it for `yak help-env`) and the soft error macros.
- `app/yak_util`, `app/yak_hash` (`YakMutMap`), `app/yak_directory` (directory trees and their digests), and `app/yak_http` (HTTP client) hold shared utilities.

### Loading and Starlark

- `starlark-rust/` is the Starlark interpreter (`starlark`), its parser and AST (`starlark_syntax`), its ordered maps (`starlark_map`), the `#[starlark_module]` and `#[starlark_value]` macros (`starlark_derive`), and an LSP server (`starlark_lsp`). It depends on no yak crate.
- `app/yak_interpreter` holds the Starlark plumbing that build files, `.bzl` files, and BXL share (module paths, loaded modules, and the slots that downstream crates fill with globals).
- `app/yak_interpreter_for_build` evaluates `YAK`, `.bzl`, and `PACKAGE` files and defines the build-file globals (`rule`, `attrs`, `select`, `read_config`, and others). `InterpreterResultsKey` and `EvalImportKey` live in `src/interpreter/calculation.rs`. `AttributeSpecExt::parse_params` checks each attribute value against the rule's `AttributeSpec` and converts it to a `CoercedAttr` through `AttrTypeCoerce` (`src/attrs/coerce.rs` and `src/attrs/coerce/`).
- `app/yak_node` defines the target graph (`TargetNode`, `ConfiguredTargetNode`, `CoercedAttr`, `ConfiguredAttr`) and the traits through which other crates request nodes.
- `app/yak_external_cells` serves cells whose files come from outside the repository, from the binary (`bundled`), from Git (`git`), from a Cargo workspace (`cargo`), or from a Go module (`go`). `src/generated.rs` serves the files of the `cargo` and `go` cells, which exist in memory apart from the third-party sources, and copies a third-party package's sources into `yak-out` when a build first reads them, declaring them to the materializer. `app/yak_external_cells_bundled` embeds `prelude/` in the binary at compile time (`build.rs` under Cargo, a generated `prelude/contents.rs` under yak).
- `app/yak_external_cells_cargo` translates `cargo metadata` output into the `cargo` cell's build files: one per third-party package, a root `YAK` file of aliases, and a `workspace.bzl` whose `cargo_package` macro declares each workspace member's targets in the package of the member's directory. The macros export the files that a member includes from another generated package. It evaluates `cfg(...)` conditions against `rustc --print cfg` output. Its functions do no I/O. The `cargo` origin in `app/yak_external_cells/src/cargo.rs` runs `cargo` and `rustc` and reads the manifests through DICE. It scans the members' Rust files for `include!`, `include_str!`, and `include_bytes!` through a DICE key per file. `yak generate` reads the member list with the same crate.
- `app/yak_external_cells_go` translates `go list -json` output into the `go` cell's build files: one per third-party module version, a root `YAK` file of aliases by import path, and a `module.bzl` whose `go_module` and `go_package` macros declare the targets of the module's packages. Each package belongs to the nearest build file at or above its directory. Its functions do no I/O. The `go` origin in `app/yak_external_cells/src/go.rs` runs `go list` for each GOOS and GOARCH pair. It reads `go.mod`, `go.sum`, the module's directory listings, and a header of each Go file through DICE, so an edit that leaves imports, build constraints, and `//go:embed` lines unchanged does not run `go list` again. `yak generate` (`app/yak_client/src/commands/generate.rs`) finds the project's `go.mod` files and configures a cell for each.
- `app/yak_external_cells_starlark` writes the Starlark values of the generated build files for both translation crates.

### Configuration

- `app/yak_configured` computes configured target nodes (`ConfiguredTargetNodeKey`), target platforms, and execution platform resolution (`ExecutionPlatformResolutionKey`).
- `app/yak_transition` implements the `transition()` global and applies transitions (`TransitionKey`).
- `app/yak_cfg_constructor` implements `set_cfg_constructor()` and modifier evaluation. Only the root `PACKAGE` file can register a constructor.

### Analysis and queries

- `app/yak_analysis` runs a rule's `impl` function (`AnalysisKey`, `run_analysis`) and resolves configured attributes into `ctx.attrs` values.
- `app/yak_build_api` defines the analysis and build API. It holds `AnalysisResult`, `AnalysisRegistry` and `ActionsRegistry`, the Starlark `AnalysisContext`, providers (`ProviderCollection`, built-in providers declared with `#[internal_provider]`), Starlark artifacts, and the build entry points (`build_configured_label`, `ensure_artifact_group`, `BuildKey`).
- `app/yak_artifact` defines artifact and action identities (`SourceArtifact`, `BuildArtifact`, `ActionKey`).
- `app/yak_anon_target` implements anonymous targets and promise artifacts.
- `app/yak_bxl` implements BXL, Starlark scripts that inspect and build the graph (`BxlKey`).
- `app/yak_query_parser` parses query expressions, `app/yak_query` evaluates them over any `QueryEnvironment`, and `app/yak_query_impls` implements the uquery, cquery, and aquery environments.
- `app/yak_validation` implements validation actions.

### Execution and materialization

- `app/yak_action_impl` implements the Starlark actions (`run`, `copy`, `write`, `write_json`, `download_file`, dynamic actions, and others). `RunAction` implements `ctx.actions.run`.
- `app/yak_execute` declares the execution interfaces. `PreparedCommandExecutor` runs a command, `Materializer` puts outputs on disk, and `RemoteExecutionClient` wraps the Remote Execution connection.
- `app/yak_execute_impl` implements them. `src/executors/` holds `LocalExecutor`, `ReExecutor`, `HybridExecutor`, the action cache checkers, and persistent workers. `src/materializers/deferred.rs` holds `DeferredMaterializer`, the only materializer. `src/sqlite/` keeps materializer, dep-file, and incremental state on disk.
- `app/yak_execute_local` spawns local processes and streams their output. On Unix the daemon spawns them through a forkserver process (`app/yak_forkserver`).
- `remote_execution/re_grpc` (package `remote_execution`) is the client for the Bazel Remote Execution API v2. `app/yak_re_configuration` reads its settings from the `[yak_re_client]` yakconfig section.
- `app/yak_resource_control` limits the memory of local actions with Linux cgroup v2. `host_sharing` limits how many local commands and tests run at once.
- `app/yak_file_watcher` reports file changes to DICE. The `yak.file_watcher` yakconfig key selects `notify` (the default in this repository), `watchman`, or `fs_hash_crawler`.

An execution platform's `CommandExecutorConfig` selects local, remote, or hybrid execution for each action.
Without an execution platform, this repository's build runs every action locally (`get_default_executor_config` in `app/yak_server/src/daemon/common.rs`).

### Tests

- `app/yak_test` is the daemon side of `yak test` (`YakTestOrchestrator`, `TestExecutionKey`). It launches a test executor process and serves the orchestrator API to it over gRPC. It keeps the passes of tests that support caching in `yak-out/<isolation dir>/cache/test_results` (`test_result_cache.rs`), and looks them up in and uploads them to a remote cache through the test's executor.
- `app/yak_test_api` and `app/yak_test_proto` define the protocol between yak and a test executor (`TestExecutor`, `TestOrchestrator`).
- `app/yak_test_runner` is the built-in test executor, which runs as `yak internal-test-runner`. yak uses it unless `[test] v2_test_executor` names another executable.

### Events and logs

- `app/yak_data` defines the event schema. `data.proto` declares `YakEvent`.
- `app/yak_events` creates and dispatches events inside a process (`EventDispatcher`, `EventSink`).
- `app/yak_event_log` writes and reads event logs. Each command writes `yak-out/<isolation dir>/log/<timestamp>_<command>_<trace id>_events.pb.zst`.
- `app/yak_cmd_log_client` implements `yak log`, which reads those files (`what-ran`, `what-failed`, `critical-path`, `replay`, and others).
- `app/yak_event_observer` aggregates events into the state the consoles render. `superconsole/` is the terminal UI library. `Cargo.toml` excludes it from the workspace and uses it as a path dependency.
- `app/yak_critical_path`, `app/yak_build_signals`, and `app/yak_build_signals_impl` compute the critical path of a build.

### Rules and libraries

- `prelude/` holds the Starlark rules and toolchains for every supported language (`prelude/cxx`, `prelude/rust`, `prelude/python`, and others). The binary embeds it as the `prelude` cell, and `yak init` configures new projects to use that copy. The prelude has no Java, Kotlin, Android, or JavaScript rules, as the owner chose on 2026-09-28 (`docs/exec-plans/completed/2026-09-29-remove-jvm-and-buck1-compatibility.md`). The `os` constraint in `prelude/os/constraints/` has an `android` value for C, C++, Rust, and Go code that targets Android.
- `gazebo/` holds small utility crates. `dupe` defines `Dupe`, a clone that is constant time and allocation-free.
- `allocative/` measures memory use per type (`Allocative`).
- `shed/` holds generic data structures that know nothing of yak (`lock_free_hashtable`, `static_interner`, `provider`, and others).
- `games/` holds terminal games that the superconsole can show during a build.

### Build and tooling

- `Cargo.toml` defines the Cargo workspace. CI builds and tests the repository with Cargo.
- `.yakconfig`, the `YAK` files, `build_defs/`, `toolchains/`, and `third-party/` build the same crates with yak (see [Two build definitions](#two-build-definitions)).
- `test.py` runs clippy, rustdoc, and the unit and doc tests. CI runs it after building the binary.
- `integrations/rust-project` generates `rust-project.json` for rust-analyzer from yak targets.
- `tools/starlark_fmt` formats Starlark and build files.
- `explorer/` is an Electron app for exploring the graph. It is not part of the Cargo workspace.
- `examples/` holds example projects, which the manually triggered `.github/workflows/build-and-examples.yml` builds.
- `tests/` holds the pytest integration tests, which `.github/workflows/integration-tests.yml` runs on Linux (see `tests/README.md`).
- `website/` holds the user documentation site, which Docusaurus builds. Its pages live in `website/docs/`, where `website/gen_docs.py` also writes the API, rule, and command reference pages that a built `yak` generates. `.github/workflows/upload_yak.yml` deploys the built site to the `gh-pages` branch on every push to `main`.
- `docs/` holds contributor documentation (`docs/developers/`), the execution plan contract (`docs/PLANS.md`), and the plans and tech-debt tracker (`docs/exec-plans/`). The site does not include it.

## Invariants and boundaries

### Late binding

A lower crate declares a `LateBinding` static (`app/yak_util/src/late_binding.rs`) and calls through it, and a downstream crate supplies the implementation at startup.
This keeps heavy crates such as `yak_bxl` out of most crates' dependency graphs, which shortens rebuilds.
Each implementing crate exposes `init_late_bindings()`, and `main` in `app/yak/bin/yak.rs` calls all of them before any thread starts.
`LateBinding::get` returns an internal error that names a binding nobody initialized.
Test crates that need bindings from several crates (such as `app/yak_build_api_tests`) initialize them with `#[ctor]`.

The same goal produces pairs of an interface crate and an implementation crate (`yak_execute` and `yak_execute_impl`, `yak_build_signals` and `yak_build_signals_impl`, `yak_query` and `yak_query_impls`, `yak_server_ctx` and `yak_server_commands`).
Code that needs the behavior depends on the interface crate.

### Crate dependency rules

`app_dep_graph_rules/rules.bzl` records these rules:

- Only `app/yak` depends on `yak_anon_target`, `yak_bxl`, `yak_cmd_audit_server`, `yak_cmd_query_server`, `yak_cmd_targets_server`, and `yak_query_impls`. They reach the rest of the program through late bindings.
- Only `app/yak` depends on `yak_cmd_debug_client` and `yak_cmd_log_client`.
- Neither crate in each of these pairs depends on the other, directly or transitively:
  - `yak_common` and `yak_directory`
  - `yak_common` and `starlark`
  - `yak_build_api` and `yak_execute_impl`
  - `yak_build_api` and `yak_interpreter_for_build`
  - `yak_server` and `yak_server_commands`
  - `yak_bxl` and `yak_configured`
- The client-only binary does not depend on the Remote Execution client. The yak build produces that binary (`yak_client-bin`, compiled with `--cfg client_only`), which leaves out the daemon, the server, and every late-binding implementation.

`//app_dep_graph_rules:test_yak_dep_graph` checks the rules during analysis, so `yak build //app_dep_graph_rules:test_yak_dep_graph` fails when a dependency breaks one. CI runs no yak build, and the tech-debt tracker records that gap.

### DICE correctness

- A computation reads files, yakconfig, and other computed values only through DICE. DICE then records the dependency and invalidates the value when the input changes. Uncached helpers such as `get_interpreter_results_uncached` must not run inside a `Key::compute`, because DICE cannot see their reads.
- A key's `equality_behavior` never reports two different values as equal. A false equality leaves dependent values stale.
- A feature gate is a key in the `[yak]` yakconfig section read through DICE. `yak_env!` environment variables are for the client before it reaches the daemon and for test-only settings.

### Other invariants

- Code calls `ensure_materialized` on an artifact's path before reading it from disk. `Materializer` in `app/yak_execute/src/materialize/materializer.rs` lists the full set of materializer invariants.
- A breaking change to a SQLite schema in `app/yak_execute_impl/src/sqlite/` bumps that database's hand-maintained schema version constant (for example `MATERIALIZER_DB_SCHEMA_VERSION`).
- Each `TargetLabel` is interned once and compared by pointer.
- `Dupe` is implemented only where `clone` is constant time and allocation-free, and `dupe` always calls `clone`.

## Where input becomes typed values

- Command-line arguments become clap structs in the client crates, which send protobuf requests from `app/yak_cli_proto` to the daemon.
- yakconfig files become `LegacyYakConfig`. Code reads a key through `YakconfigKeyRef` and parses it into a typed value at the read.
- Environment variables that configure yak are declared with `yak_env!`, which parses each into a typed value. Some code in `app/` reads other variables directly with `std::env::var`.
- Build files become Starlark values, which attribute coercion (`AttrTypeCoerce`) checks against the rule's `AttributeSpec` and converts into `CoercedAttr` values on a `TargetNode`.
- Target pattern strings become `ParsedPattern` values.
- Path strings become `ForwardRelativePath`, `ProjectRelativePath`, or `AbsNormPath` through checked constructors that reject `.` and `..` components. Each of these types also has an `unchecked_new` for a string the caller has already validated.

## Cross-cutting concerns

### Errors

Fallible functions return `yak_error::Result`.
Errors carry tags (`ErrorTag`) that classify them, and `internal_error!` marks a bug in yak itself.
`docs/developers/error_handling.md` covers defining, tagging, and converting errors.

### Observability

The daemon reports progress as `YakEvent`s, which the client renders and writes to the event log.
`yak log` reads the event log, and `YAK_LOG` enables `tracing` output.
`docs/developers/debugging.md` lists the commands.

### Build flags

Two `cfg` flags change what a crate compiles:

- `#[cfg(yak_build)]` code compiles only under yak. The macros in `build_defs/rust.bzl` set the flag. Under Cargo, `app/yak_external_cells_bundled` embeds the prelude through `build.rs`, and `app/yak/bin/yak.rs` sets jemalloc as the global allocator on Linux and macOS.
- `#[cfg(client_only)]` code compiles only in the client-only binary (see [Crate dependency rules](#crate-dependency-rules)).

### Two build definitions

Each crate has a `Cargo.toml` and a `YAK` file, and a change to its dependencies updates both.
The `YAK` files load their Rust macros from `build_defs/rust.bzl` and `build_defs/proto.bzl`, and they name crates in this repository as `//<path>:<crate>` (for example `//app/yak_core:yak_core`).
They name third-party crates as `//third-party/rust:<crate>`, which this repository no longer defines, so the yak build of this repository does not load.
`toolchains/YAK` defines the toolchains the build uses. `third-party/proto/` provides `protoc`, and `third-party/win/` provides the Windows libraries.
`.yakconfig` sets `[external_cells] prelude = bundled`, so a yak build of this repository loads the prelude embedded in the running binary. Changes to `prelude/` take effect after a rebuild of the binary.

### Memory

Cargo builds on Linux and macOS use jemalloc, and Windows builds use mimalloc (`#[global_allocator]` in `app/yak/bin/yak.rs`).
`allocative` attributes memory to types, and `docs/developers/perf/memory.md` covers heap profiling.

## Planned changes

- The owner plans to remove what still ties the repository to Meta's upstream projects, such as the downloads from upstream releases. `docs/exec-plans/tech-debt-tracker.md` lists them under Upstream connections.
