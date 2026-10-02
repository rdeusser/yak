# Yak

## Unreleased

Removes the code, configuration, and service clients that only Meta's internal build used, removes the JVM, Android, and JavaScript support and the Buck1 compatibility code, and renames the tool to yak.

### Cargo workspaces

- `yak generate` writes the build files of a Cargo workspace: a three-line `YAK` file next to each member's `Cargo.toml` and at the root of the workspace, and the `crates` cell in `.yakconfig`. Without a `.yakconfig`, it also writes the files of `yak init`.
- The `cargo` cell applies the `build.rustflags` and `target.<triple or cfg>.rustflags` of Cargo's configuration files to every crate of the workspace and of the cell, and passes them to `rustc --print cfg`. It ignored them before, so a workspace that set `--cfg tokio_unstable` in `.cargo/config.toml` built tokio without its `cfg(tokio_unstable)` API and dependencies. An edit of a configuration file in the project computes the cell again.
- The `cargo` cell compiles each crate with the settings of the workspace's `[profile.dev]`, from its `Cargo.toml` and Cargo's configuration files: `opt-level`, `debug`, `debug-assertions`, `overflow-checks`, and `codegen-units`, with the `build-override` and `package` tables. It passed none of them before, so crates compiled without optimization or debug information. With `panic = "abort"`, a member's binaries and their dependencies compile with `-Cpanic=abort` through the transition `prelude//rust/panic:panic_transition[abort]`, and tests keep unwinding. A build script gets `OPT_LEVEL`, `DEBUG`, and `PROFILE` from the settings of the crate it builds for.
- `yak generate` writes no `YAK` file and no cell in a directory that `[project] ignore` lists, where yak reads no build file. It wrote them for the Go modules in such directories before.
- The `cargo` external cell origin generates, from `cargo metadata`, a package for each third-party package of a workspace and the `cargo_package` macro that declares a workspace member's targets in the package of its directory, such as `//crates/server`. A member's library and binaries list its tests in `tests`, so `yak test //crates/server` runs them. `[external_cell_<name>] manifest` names the workspace's `Cargo.toml`.
- A crate builds with the files that its Rust files name in `include!`, `include_str!`, and `include_bytes!`, which the cargo cell finds in their sources. The package that owns such a file exports it when the cell declares that package. A crate that reads other files of other packages names their targets in `include` of `cargo_package()`, and a crate whose tests read them at run time names them in `test_data`.
- The Rust rules take `srcs_path`, the directory that the root of a crate's source tree stands for in `file!()`, panic locations, and debug information. A workspace member's crates set the workspace's directory, so they name their files as with Cargo, such as `crates/server/src/main.rs`. They named them with the member's directory twice before, such as `crates/server/crates/server/src/main.rs`.
- The Rust rules take `package_srcs`, which places the files of targets in other packages in a crate's source tree, below the directory of their package.
- `include` and `test_data` of `cargo_package()` take targets of other cells whose directories are inside the workspace's directory. `package_srcs` places the files of a `source_listing` at their paths relative to its package, so `prelude//:source_listing` gives every file of the `prelude` cell.
- The `cargo` cell builds each third-party package from the sources that Cargo downloaded, so packages from private registries, replaced sources, vendored directories, and Git build as they do with `cargo build`.
- `buildscript_run` sets `CARGO_PKG_VERSION_MAJOR`, `CARGO_PKG_VERSION_MINOR`, `CARGO_PKG_VERSION_PATCH`, `CARGO_PKG_VERSION_PRE`, `DEBUG`, `NUM_JOBS`, `PROFILE`, and `RUSTDOC` for build scripts, as Cargo does. It sets `OPT_LEVEL` always, to `0` when the toolchain's flags set no optimization level.
- When a build script fails, `buildscript_run` prints the script's stdout, where `cargo::error=` messages go.
- `buildscript_run` keeps a `cargo:rustc-link-search` directory outside `OUT_DIR`, such as the one where pkg-config found a system library, and passes it to the links of the libraries that depend on the run target. It dropped the directory before, so a link that needed the library failed with `library not found`.
- `buildscript_run` copies the shared libraries in the `cargo:rustc-link-search` directories of `OUT_DIR` into its `shared_libs` output, and passes that directory to the links of dependents. A Rust binary gets an rpath to the `shared_libs` directory of every build script among its dependencies, so it loads those libraries under `yak run` and `yak test`.
- On Linux, the C compiler that `buildscript_run` gives build scripts links executables and shared libraries with a clang that defaults to PIE, such as Debian's. Every link failed before, because the `LD` that the compiler links through let the clang driver add its own startup files and `-pie`, and `ld.lld` rejected `-pie` with `-shared`.
- The C compiler that `buildscript_run` gives build scripts runs with Python 3.11. It failed before with `TypeError: PurePath.relative_to() got an unexpected keyword argument 'walk_up'`, an argument that Python 3.12 added.
- The C compiler that `buildscript_run` gives build scripts can be named without a path, such as `clang` in `system_toolchains`. It failed with `FileNotFoundError` before, because the wrapper ran it without searching `PATH`.
- A workspace member's build script runs in the member's directory of a copy of the workspace's layout that holds the member's files and the files of `include`. It ran in a copy of the member's own files before, so a script that read `../proto/api.proto`, as protobuf build scripts of sibling crates do, failed, although `include` named the file. `buildscript_run` takes `package_srcs` and `manifest_subdir` for this.
- The build script of a package gets the `DEP_<links>_<key>` variables of the metadata that the build scripts of its normal dependencies with `links` print, as with Cargo. `aws-lc-rs` failed before with `missing DEP_AWS_LC_ include`. `buildscript_run` takes the run targets of those dependencies in `links_deps`, and writes the metadata of its script to its `metadata` sub-target.
- `buildscript_run` removes the symlinks in the build script's current directory after the script exits, and resolves the paths under `CARGO_MANIFEST_DIR` that the script prints to the files they name. The local action cache persisted no build script run before, because the output held symlinks to the action's inputs, so every build script ran again after the daemon restarted.
- A workspace member's build script and its aliases for each Cargo platform build only for the execution platform of the script's run target. `yak build //...` built them for the target platform and for each of 9 Cargo platforms before, which was 35% of the actions of a clean build of a workspace with 775 packages.
- `yak test //...` in a Cargo workspace runs the tests that `cargo test` runs, as `cargo test` runs them. `cargo_package()` declares the unit tests of binaries and a target for each example. A member's test binary sits in a `deps/` directory beside its package's binaries and examples, as in Cargo's `target/debug/`. The test runs in a copy of its package's files, its included files, and its `test_data` files, from the package's directory there, and `CARGO_MANIFEST_DIR` names that directory at compile time and at run time. A test that reads a file it does not declare fails. An integration test gets `CARGO_BIN_EXE_<binary>`. A target whose `required-features` are off, or with `test = false`, gets no target or test.
- `rust_test` takes `cargo_target_files`, the files at their paths in Cargo's profile directory, which puts the test binary in `deps/`. It takes `run_from_manifest_dir`, which runs the test from the directory that `CARGO_MANIFEST_DIR` names in the crate's copy of its sources, with absolute paths.
- The `CARGO_MANIFEST_DIR` that a `rust_test` gets at run time names the same directory as at compile time, in the crate's copy of its sources. It was the value of `env` before, a path that did not resolve.
- `ExternalRunnerTestInfo` takes `working_directory`, the directory a test runs in.
- A build or test of a pattern does not list the targets it skips because their `target_compatible_with` requires the execution platform marker (`[build] exec_platform_marker`), such as a workspace member's build script. Those targets build only as exec dependencies. Other incompatible targets are listed as before.

### Go modules

- The `go` external cell origin generates, from `go list`, a package for each third-party module version of a Go module and the `go_module` and `go_package` macros that declare the targets of the module's packages. `[external_cell_<name>] module` names the module's `go.mod`. `go_module()` in the build file next to `go.mod` declares a library, binary, or test target for each package, and `go_package()` declares them in a build file below the module's root. Third-party packages are aliases named by import path, such as `gomod//:golang.org/x/sys/unix`, and their dependencies select on the operating system and CPU where Go's build constraints differ.
- The `go` cell runs `go list` again only when `go.mod`, `go.sum`, the names of the module's files, or the imports, build constraints, package clauses, or `//go:embed` lines of its Go files change.
- A test of a `go` cell runs in its package's directory with the files of that directory, apart from other packages' directories, as with `go test`. `go_module(test_data = {...})` and `go_package(test_data = {...})` declare the other files a package's tests read.
- `yak generate` writes a three-line `YAK` file next to each `go.mod` below the directory it runs in, and a go cell for each module in `.yakconfig`. It runs without a `Cargo.toml` when the directory holds Go modules.
- `yak test --changed-since` tests the targets that load a `go` cell's `module.bzl` when a Go file, `go.mod`, `go.sum`, or a build file in the module changes, or when a file appears or disappears in the module.

### Testing

- `go_test` takes `working_directory`, the directory below its resources that the test runs in. `go_test` takes `external_tests_only`, which links the package of `target_under_test` as that target builds it, as `go test` does for a package without internal tests. A dependency of the external tests that imports the package failed to link with `conflict for package` before.
- `go_test` copies its `resources` into one directory next to its binary and runs the test there. A resource that the target no longer listed stayed in its output directory before, so a test could keep reading a file it did not declare. A test whose name contains `/` did not find its resources, because the test ran in the directory of its binary, which such a name put below them.

- `go_test` builds external tests, the test files that declare `package <name>_test`, into a package of their own that imports the package under test with its own test files, as `go test` does. It failed with `External tests are not supported` before.
- `yak test` reports an earlier pass of a test that the `cargo` or `go` cell declares in place of running it again when the test command, its environment, and the contents of its inputs are unchanged. The passes stay in `yak-out/<isolation dir>/cache/test_results` across daemon restarts. A cached pass prints as `✓ Pass (cached)`, and `yak log what-ran` lists it with the `local_cache` executor. `yak test --no-test-cache` runs every selected test.
- The action digest of a test whose command or environment names paths by absolute path covers them with `${YAK_PROJECT_ROOT}` in place of the project root. The digest of such a test, such as one that the `cargo` or `go` cell declares, depended on the path of the checkout before.
- With a remote cache, `yak test` uploads each pass of a local run of a test that supports caching, and looks it up on other machines. A test whose own executor is local-only, as the executor that the prelude gives a test without a remote execution profile is, uses the remote cache of its execution platform. Tests never uploaded their results before.
- `[build] remote_cache` (`off`, `read`, or `read_write`) configures the remote cache of `prelude//platforms:default`.
- A `BatchUpdateBlobs` request counts the digest and framing of each blob toward the batch size. A batch of thousands of small files exceeded a server's 4 MiB message limit before, which failed the action under `--upload-all-actions` with `received message larger than max`.
- `[yak_re_client] engine_address` is optional. A remote cache without remote execution needs only `action_cache_address` and `cas_address`, and yak fetches the server's capabilities from `cas_address`. The client failed with `No engine address` before.
- `rust_test` and `go_test` take `supports_test_execution_caching`, which defaults to `False`. The `cargo` and `go` cells set it to `True`.
- `yak test --changed-since <revision>` tests only the matched targets that the changes since a Git revision can affect. The changes are those between the working tree and the merge base of the revision and `HEAD`. A change to the configuration, a submodule, a configuration target, `rust-toolchain.toml`, or a path in `[test] changed_since_select_all` tests every matched target.

### File watching

- When the operating system drops file system events, the `notify` file watcher crawls the project and invalidates the files whose size, modification time, or status change time differ from its previous crawl. It cleared the DICE graph before, so the next build analyzed every target again. On macOS, the outputs that the build rewrote made FSEvents drop events again, so a no-op build of a workspace with 775 packages took 4 seconds, and it now takes 0.05 seconds.
- On macOS, the file watcher reads FSEvents directly and leaves `yak-out` out of the stream.
- The `notify` file watcher invalidates the paths under a renamed, removed, or replaced directory. FSEvents and inotify report such a directory as one event, and a query of a package under the old path answered from the cached listing and build file until the daemon restarted.
- The `notify` file watcher writes a sync marker file at the start of each command and waits for its event, so the command sees every change made before it started. FSEvents delivers events about 12 milliseconds after a change on macOS, and a command that started within that time could build the old contents of a file.
- `yak.file_watcher` defaults to `auto`, which selects Watchman when `WATCHMAN_SOCK` is set or `watchman` is on the daemon's `PATH`, and the `notify` watcher otherwise. The default was `notify`. If an auto-selected Watchman fails, the error names `yak.file_watcher = notify`.
- The Watchman watcher sends its query at the start of each command's DICE update, so Watchman's wait for its sync cookie overlaps the configuration loading. A no-op build of `//gazebo/dupe:dupe` in this repository took 58 ms with Watchman and 44 ms with `notify` before, and both take 46 to 48 ms now.
- The Watchman query leaves out the files under `yak-out`.
- `yak init` writes a `.watchmanconfig` that lists `yak-out` in `ignore_dirs` when the project has none, and so does `yak generate` when it creates a project.

### Daemon

- The daemon runs `rustc -vV`, `go version`, and `clang --version` in the project root at the start of each command and sets `tool_identity.rustc`, `tool_identity.go`, and `tool_identity.clang` to digests of their output. The toolchains of `system_toolchains()` take a `tool_identity` and put it into the key of every action that runs their tools, so a compiler upgrade runs those actions again, and a remote cache keeps the results of different compilers apart. A build after a compiler upgrade reused the results of the old compiler before.
- The `cargo` cell runs `cargo metadata` again when `tool_identity.rustc` changes, and the `go` cell runs `go list` again when `tool_identity.go` changes. They kept the packages of the old toolchain before.
- The daemon writes `yak-out/go.mod`, so `go build ./...`, `go test ./...`, and `go list ./...` in a Go module at the project root skip `yak-out`. They failed before on the Go sources that builds copy there.
- The local action cache persists across daemon restarts by default (`[yak] sqlite_dep_file_state`). A build after `yak kill`, or after a new `yak` binary restarted the daemon, ran every command again before. `sqlite_dep_file_state = false` turns it off.
- After `yak kill`, or after the daemon exited on its own, the next command prints `Starting new yak daemon...`. It printed `Could not connect to yak daemon (yak daemon is not running), killing daemon..` before, because `yakd.info` outlives the daemon.
- On Linux and macOS, the local actions of a daemon stop when `yak kill` stops the daemon or when a signal kills it. The forkserver exits when the daemon's socket closes, and it now kills the process group of each action it still runs. The actions and the processes they started kept running before, and a run of the integration tests left 115 of them.
- `yak killall` kills the yak processes of the current repository, and `yak killall --global` (`-g`) kills those of every repository. It killed the processes of every repository before, and `--repo` limited it to the current one. Outside a repository, `yak killall` fails and names `--global`.
- `yak killall` kills the process groups of the local actions that the killed processes started. It sends `KILL` to the forkserver, which then cannot stop them.

### Labels

- A label in a build or `.bzl` file that omits the target name names the target named after its package's directory, so `//lib/greeting` is `//lib/greeting:greeting`, as on the command line. It was an error before. `[yak] infer_target_names = false` turns it off.

### Renamed to yak

- The binary is `yak`, and `cargo build --bin=yak` builds it.
- yak reads build files named `YAK`.
- `system_toolchains()` in `@prelude//toolchains:system.bzl` declares a toolchain for each language, using the tools on `PATH`. It was `system_demo_toolchains()` in `demo.bzl`, and a `toolchains/YAK` file that loads the old name fails to load.
- `[buildfile] name` lists the exact build file names to read. The `name_v2` key and the `.v2` variant of each name are gone.
- The project configuration files are `.yakconfig`, `.yakconfig.local`, and `.yakconfig.d/`.
- The global configuration is in `/etc/yakconfig` and `/etc/yakconfig.d/`, or in `C:\ProgramData\yakconfig` and `C:\ProgramData\yakconfig.d` on Windows.
- A `.yakroot` file marks the project root.
- The settings files are `.yaksettings.toml` and `.yaksettings.local.toml`.
- Build output goes to `yak-out`.
- The reserved directory in `yak-out` is `._yak`. Tools keep scratch files in `yak-out/._yak/tmp`, and `--isolation-dir` rejects names that start with `._yak`.
- The daemon keeps its state in `~/.yak/yakd/`, in files named `yakd.info`, `yakd.pid`, and so on.
- The alternative package file name is `YAK_TREE`.
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
- The environment variables that yak reads take the `YAK_` prefix, such as `YAK_LOG` and `YAK_ISOLATION_DIR`.
- The daemon startup timeouts are `YAKD_STARTUP_TIMEOUT` and `YAKD_STARTUP_INIT_TIMEOUT`.
- Actions and the prelude's tools see `YAK_SCRATCH_PATH`, `YAK_BUILD_ID`, and the other variables that yak and the prelude set.
- The configuration sections are `[yak]`, `[yak_re_client]`, `[yak_resource_control]`, `[yak_system_warning]`, `[yak_hydration]`, and `[yak_metadata]`.
- The hidden flag that runs the daemon in the client process is `--no-yakd`, and its variable is `YAK_NO_YAKD`.
- `host_info()` no longer has a `buck2` field.
- The integration tests take the binary from `YAK_BINARY` and rewrite golden files when `YAK_UPDATE_GOLDEN` is set.
- The documentation calls the tool yak and its configuration the yakconfig.
- The site's pages `concepts/buckconfig`, `concepts/buck_out`, `concepts/buck_query_language`, `getting_started/what_is_buck2`, and `users/faq/buck_hanging` moved to `concepts/yakconfig`, `concepts/yak_out`, `concepts/query_language`, `getting_started/what_is_yak`, and `users/faq/yak_hanging`.
- The site no longer has the page that compared Buck2 with Buck1, or the lists of articles, videos, projects, and tools about Buck2.
- The site's logo is a yak.
- `website/gen_docs.py` takes the path of the binary with `--yak`.
- The publisher of the Starlark extension for VS Code is `yak`.
- The messages, help text, and doc comments of the binary and the prelude call the tool yak and its configuration the yakconfig.
- HTTP requests name `yak` as the user agent.
- The Visual Studio projects that `vsgo` generates build with `yak`. They ran `buck2`.
- The example target in the `YAK` file that `yak init` writes prints `BUILT BY YAK`.
- The `rust-project.json` that `rust-project` writes tells rust-analyzer to run tests with `yak test`.
- The examples of the query functions in `yak docs uquery` query this repository's own targets.
- `docs/developers/perf/scripts/bin_waste.py` takes the path of the binary with `--yak`.
- The workflows that build and upload the binaries are `.github/workflows/build_yak.yml` and `.github/workflows/upload_yak.yml`, and their version input is `yak_version`.
- Threads take yak names, such as `yak-main`, `yak-rt`, and `yak-dm`, which thread dumps and panic messages print.
- The forkserver's process name is `(yak-forkserver)`.
- `yak build --out` names its temporary file with the suffix `.yak.tmp`. It writes that file when the destination is a running executable.
- Workers listen on sockets under `/tmp/yak_worker`.
- The games save their state in `~/.yak_games`.
- The default Remote Execution use case is `yak-default`.
- The uploads of `[yak] clean_stale_unmaterialize_upload_enabled` use the Remote Execution use case `yak-local-unmaterialization`.
- The Cargo packages and their directories take `yak` names, such as `yak_core` in `app/yak_core`. The package that builds the binary is `yak` in `app/yak`, and `cargo install --path=app/yak` installs it.
- `YAK_LOG` filters name modules by the new crate names, such as `yak_execute_impl::materializers=trace`.
- The error derive macro reads `#[yak(...)]` attributes.
- The `yak_bundle` rule in `defs.bzl` takes the binaries as `yak` and `yak_client`.

### Removed JVM, Android, and JavaScript support

- The Java, Kotlin, and Android rules and toolchains are removed.
- `system_toolchains()` no longer defines the Java, Kotlin, Android, and dex toolchains.
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
- The hidden `--skip-targets-with-duplicate-names` flag is removed. Its help called it a hack for TD and said not to use it.

### Removed analytics

- The Scribe, Scuba, and Manifold clients are deleted, together with every code path that sent events, logs, or metrics to them.
- The client no longer adds an `id` entry to `--client-metadata` for analytics.

### Event log

- `ActionExecutionEnd` no longer records the host name of a local action.
- The instant events `RageResult`, `IoProviderInfo`, `PersistEventLogSubprocess`, `ActionDigestTrace`, and `ReLogStreamAvailable` are removed. On an event log from an earlier version that holds one of them, `yak log replay` and `yak log whatup` fail with ``Missing `data` in `Instant` ``.

### Removed configuration

- `yak init` no longer writes the `none` cell or the `fbcode`, `fbsource`, `fbcode_macros`, `buck`, and `ovr_config` cell aliases. A project that loads through those names needs its own `[cell_aliases]` entries.
- The `[buck2_health_check]` section is removed.
- The `[scuba] defaults` key is removed.
- The `[agent_context] enforced_clients` key is removed.
- The `[buck2]` keys for Eden are removed (`allow_eden_io`, `detect_eden_restart`, `use_eden_thrift_read`).
- The `[buck2]` keys for event log upload are removed (`log_use_manifold`, `event_log_buffer_size`, `event_log_message_batch_size`, `event_log_retry_attempts`, `event_log_retry_backoff_duration_ms`).
- The `log_use_manifold` setting in `[log_download]` is removed. `log_url` still selects a server for `yak log` downloads.
- The `[buck2]` keys `agent_hostname_fail_v2_context`, `agent_hostname_fail_v2_glob`, and `allow_daemon_start_unsandboxed_via_wrapper` are removed.
- The `[apple] info_plist_identify_build_system` key is removed.
- The `[http] proxy_env_allowlist` key is removed. Only Meta's internal HTTP client read it. `HTTPS_PROXY`, `HTTP_PROXY`, and `NO_PROXY` work as before.
- The environment variables for Eden (`BUCK2_DISABLE_EDEN_HEALTH_CHECK`, `BUCK2_EDEN_SEMAPHORE`, `BUCK2_ENABLE_EDEN_THRIFT_READ`) are removed.
- The environment variables for log upload (`BUCK2_SCRIBE_CATEGORY`, `BUCK2_TEST_MANIFOLD_TTL_S`, `BUCK2_TEST_MANIFOLD_CHUNK_BYTES`, `BUCK2_TEST_BLOCK_ON_UPLOAD`, `BUCK2_TEST_DISABLE_LOG_UPLOAD`) are removed.
- The CI metadata variables that tagged events for analytics (`SANDCASTLE`, `SANDCASTLE_ALIAS`, `SANDCASTLE_ID`, `SANDCASTLE_INSTANCE_ID`, `SANDCASTLE_JOB_INFO`, `SANDCASTLE_SCHEDULE_TYPE`, `SANDCASTLE_TYPE`, `SCHEDULE_TYPE`, `SKYCASTLE_WORKFLOW_ALIAS`, `SKYCASTLE_WORKFLOW_RUN_ID`) are no longer read.
- `BUCK2_ACTION_DIGEST_TRACE_LG_SAMPLE_RATE`, `BUCK2_DICE_DUMP_ON_PANIC`, `BUCK2_DUMP_FBS`, `BUCK2_IGNORE_VERSION_EXTRACTION_FAILURE`, and `BUCK2_TEST_DAEMON_ORIGINATING_CGROUP` are removed.
- The `YAK_TEST_EXECUTOR_USE_TCP` environment variable replaces `BUCK2_TEST_TPX_USE_TCP`.
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

- Help text and error messages link to https://rdeusser.github.io/yak/.

### Prelude

- C++ header units compile with `-DPRELUDE_CPP_HEADER_UNIT=1` in place of `-DFACEBOOK_CPP_HEADER_UNIT=1`.
- Late-stamped build info goes into an ELF section named `build_info` in place of `fb_build_info`.
- The `tests.disable_re_tests` yakconfig key replaces `fbcode.disable_re_tests`.
- A Remote Execution test without a `use_case` no longer falls back to the `tpx-default` use case.
- `YakconfigBackedModifier` no longer has an `oncall` field.
- Test rules no longer add the labels that only Meta's Tpx test runner read (the `tpx:*` labels of `apple_test`, and the `run_as_bundle` label).
- `go_test` no longer sets `TPX_LIST_TESTS_COMMAND`.
- `python_needed_coverage_test` no longer sets `TEST_PILOT`.
- `yak run` on a `go_test` without `env` runs the test binary directly. It ran through `inject_test_env.py` before.
- The `TestListingInfo` provider and the `prelude//go/tools:list_tests` target are removed.
- The `@prelude//apple:apple_test_device_types.bzl` module is removed.
- Top-level `.app` bundles no longer carry the `FBBuck2` key in `Info.plist`.
- `@prelude//cfg/modifier:alias.bzl` exports its struct of modifier aliases as `ALIASES` in place of `OSS_ALIASES`.

Rule attributes and toolchain fields that only Meta's build used are removed:

- Apple rules lose `bundle_telemetry_logger`, `entitlements_verification_check_enabled`, `_fast_adhoc_signing_probe_enabled`, `info_plist_identify_build_system`, `_info_plist_identify_build_system_default`, `_meta_apple_library_validation_enabled`, and `_sanitizer_compatibility`. `apple_test` loses `test_device_type`. `AppleToolchainInfo` loses `bundle_telemetry_logger`.
- C++ rules lose `use_fbcc_rust_wrapper`.
- `cxx_toolchain` loses `_dumpbin_toolchain_path`.
- `CxxToolchainInfo` loses `compiler_with_wrapper`.
- `http_archive` and `http_file` lose `vpnless_urls`.
- `remote_file` loses `vpnless_url`.
- Python rules lose `use_rust_make_par`, `use_rust_make_par_optimizations`, and `opt_by_default_enabled`. `PythonToolchainInfo` loses `gen_lpar_bootstrap`, `make_py_package_live`, and `manifest_module_entries`.

## Initial version

- Initial version.
