# See every file change, and make the daemon fail and clean up promptly

Follows [the plan contract](../../PLANS.md).

## Purpose

After this change, a command sees every file change made before it started, including a change made milliseconds earlier, and a renamed directory stops answering at its old path. `yak.file_watcher` selects Watchman when it is installed and the notify watcher otherwise. `yak kill` stops the daemon's running local actions. A daemon that fails to start reports its error within seconds. A command whose platform enables remote execution without a configured backend fails at once. A command no longer runs `rustc`, `go`, and `clang` when they have not changed.

The repository owner asked on 2026-10-02 for one plan that fixes the 6 tech-debt entries below, and for Watchman as the file watcher when it is installed, after the notify watcher is fixed.

| Tech-debt entry | Milestone |
| --- | --- |
| A command can miss a file changed just before it starts | 2 |
| A running daemon keeps the packages of a renamed directory | 1, 3 |
| Local actions outlive a killed daemon | 5 |
| A failed daemon start waits out the startup timeout | 6 |
| A daemon without a Remote Execution backend waits 45 seconds to fail | 7 |
| Every command waits for the tool identities | 8, 9 |

To see it working, run the reproductions of those entries in `docs/exec-plans/tech-debt-tracker.md`. Each one gives the expected result, and the tests of each milestone check the same behavior.

## Progress

- [x] Milestone 1 (prototype): choose how a renamed or removed directory invalidates the paths under it (2026-10-02). The snapshot design fixed the reproduction on macOS and Linux, and a no-op `yak build //gazebo/dupe:dupe` took a median of 44.0 to 45.3 ms with and without it over 3 runs of 30 builds each.
- [x] Milestone 2: the notify watcher waits for a sync marker before it reports changes (2026-10-02). `sync_marker_waits_for_earlier_changes` in `app/yak_file_watcher/src/notify.rs` fails at its first sync without the marker and passes 200 syncs with it. `sync_markers_in_vcs_dir` covers the marker's directory, other daemons' markers, and the removal of replaced and stale markers. `test_modify_genrule_notify` passed 20 runs in a row on macOS. `python3 test.py yak_file_watcher` and the tests of `tests/core/io`, `test_modify.py`, and `test_symlinks.py` pass on macOS and Linux. A no-op `yak build //gazebo/dupe:dupe` took a median of 44.4 ms against 44.1 ms before the change.
- [x] Milestone 3: a renamed or removed directory invalidates every path under it (2026-10-02). `test_apply_rename_of_directory` and `test_apply_removal_and_replacement` in `app/yak_file_watcher/src/rescan.rs` cover the snapshot. `test_notify_rename_parent_directory` fails with the binary before the change and passes after it.
- [x] Milestone 4: `yak.file_watcher` selects Watchman when it is installed (2026-10-02). `test_selection` in `app/yak_file_watcher/src/file_watcher.rs` covers the parsing. `tests/core/io/test_auto_file_watcher.py` checks that `auto` selects Watchman when it is on `PATH`, selects the notify watcher when it is not, and names `yak.file_watcher = notify` when the selected Watchman fails. `test_watchmanconfig` in `app/yak_client/src/commands/init.rs` covers the `.watchmanconfig` of `yak init`. The tests of `tests/core/io`, `tests/core/init`, `test_modify.py`, and `test_symlinks.py` pass on macOS with Watchman installed, and `cargo test -p yak_file_watcher -- --include-ignored` passes the Watchman unit test. `python3 test.py yak_file_watcher yak_client` passes on Linux, where Watchman is not installed and the tests of `auto` select the notify watcher.
- [x] Milestone 5: killing the daemon stops its local actions (2026-10-02). `test_stopping_the_daemon_stops_local_actions` in `tests/core/daemon/test_daemon.py` starts an action that starts a child, stops the daemon with `yak kill`, `yak killall`, or `KILL`, and checks that both processes exit within 10 seconds. All 3 cases fail with the binary before the change, and the `killall` case fails without the group kill of `killall`. The tests of `tests/core/daemon` and `tests/core/kill` and `python3 test.py yak_execute_local yak_wrapper_common` pass on macOS and Linux, and the tests of `tests/core/daemon`, `tests/core/kill`, and `tests/core/executor` left no action running on macOS.
- [x] Milestone 6: the client reports a failed daemon start when the daemon exits (2026-10-02). `test_daemon_startup_error`, `test_daemon_startup_signal`, `test_daemon_crash`, `test_daemon_killed`, and `test_daemon_abort` in `tests/core/build/test_error_categorization.py` each finish in 1.1 seconds or less with the binary before any change of this milestone, so item 1 needed no change. `test_init_data_timeout` takes 15 seconds with the daemon's sleep at 15 seconds, and took 120 seconds before. The full integration suite passes on macOS (1864 passed, 208 skipped, 3 expected failures).
- [x] Milestone 7: a missing Remote Execution address fails without retries (2026-10-02). `test_remote_only_without_address_fails_promptly` in `tests/core/executor/test_hybrid_executor.py` fails after 45 seconds with the binary before the change and passes in 0.1 seconds after it. `python3 test.py remote_execution yak_execute` passes on macOS and Linux, and the tests of `test_hybrid_executor.py`, `test_remote_execution.py`, and `test_error_categorization.py` pass on Linux. The full integration suite on macOS took 937 seconds, against 1169 seconds before the change. 3 of its tests failed and passed on a rerun, among them `test_many_rebound_outputs_incremental_rebuild`, which takes 103 seconds alone and passed its 600-second timeout under the load of the suite.
- [ ] Milestone 8 (prototype): decide whether a cache of tool identities can see every toolchain change.
- [ ] Milestone 9: commands skip the identity commands when the tools have not changed.

## Surprises & Discoveries

- `app/yak_file_watcher/src/watchman/` already implements a Watchman watcher, which `yak.file_watcher = watchman` selects. `tests/core/io/test_watchman.py` runs when `watchman` is on `PATH` and passed in a full run of the integration suite on macOS on 2026-10-02.
- Watchman queries wait for a sync cookie by default, so the Watchman watcher does not have the race of milestone 2. `SyncableQuery::new` leaves `sync_timeout` at its default (`app/yak_file_watcher/src/watchman/core.rs`).
- `tests/e2e_util/yak_workspace.py` writes a `.watchmanconfig` with `ignore_dirs` of `yak-out`, `.git`, and `.hg` into every test project.
- A query that walks down from an unchanged parent sees a renamed directory, because the rename invalidates the parent's listing. A glob of `files/**/*` from the root package after `mv files other` returned no files with the binary before the fix. Only a query that starts at the old path, such as `yak targets root//files/d:`, read the stale listing.
- Watchman reports the paths under a renamed directory. `test_watchman_rename_parent_directory` and `test_fs_hash_crawler_rename_parent_directory` pass with the binary before the change.
- An event that names a directory can stand for a replacement of the directory by another one, which leaves a directory at the path. Comparing only whether the path was and is a directory missed the paths of both, so `Snapshot::apply` compares the whole subtree under each path that an event names.
- FSEvents delivers the event of a marker about 12 ms after the write on macOS, so a sync that waited for its marker made a no-op build take 57 ms instead of 45 ms. `FSEventStreamFlushSync` does not shorten the wait: after a flush, the event of 1 write in 200 had arrived.
- The loop of the tracker entry missed no change in 200 commands with the binary before the change or after it. Client startup and configuration loading usually take longer than the FSEvents delay, so the race needs a command that reaches the sync within about 12 ms of the change.
- `tests/e2e_util/yak_workspace.py` already set `yak.file_watcher = fs_hash_crawler` for every test project. A project's `.yakconfig` overrides it, which is how the tests of `tests/core/io/` select their watcher, so `auto` changes no other test.
- The daemon reads `yak.file_watcher` from the configuration files when it starts, so `-c yak.file_watcher=notify` does not select a watcher. A benchmark that passed `-c` measured Watchman for both watchers. The measurements below set the watcher in `.yakconfig.local`.
- When an update changes the DICE state while an earlier state is still active, `yak_concurrency` cleans up the earlier state and runs the update again, so a command after a file change syncs its file watcher twice.
- Watchman's sync cookie waits for FSEvents like the notify watcher's marker, about 12 ms per sync on macOS. Before the prefetch, a no-op `yak build //gazebo/dupe:dupe` took a median of 58 ms with Watchman and 44 ms with notify, and a rewrite of `gazebo/dupe/src/lib.rs` followed by `yak targets //gazebo/...` took 106 ms with Watchman and 80 ms with notify.
- The notify watcher ignores a change of only the modification time (`ignore_event_kind` in `notify.rs`), so `touch` invalidates nothing, but Watchman reports it.
- Watchman evaluates a `dirname` term relative to the query's `relative_root`, so the `yak-out` term matches a project below the watched root. The term leaves out the files under `yak-out` and keeps the directory, whose event the ignore set drops.
- On Unix, the daemon runs local actions through the forkserver by default, and the forkserver exits when the daemon's socket closes, whether the daemon shut down or a signal killed it. Before the forkserver exits, its runtime drops the futures of the actions it still ran, without a kill, so each action kept running with init as its parent. 115 such processes from earlier runs of the integration tests were running on the macOS machine, the oldest started 2 days before.
- The 120 seconds of `test_init_data_timeout` were its teardown. The test harness sets `YAKD_STARTUP_TIMEOUT` to 120 seconds (`tests/e2e_util/yak_workspace.py`), and the teardown's `yak kill` waited that long to connect to the daemon, which slept 120 seconds before it served. The default timeout is 10 seconds, after which `yak kill` sends `KILL` to the daemon.
- `test_init_data_timeout` failed after milestone 4, because its golden file records the daemon's startup log, and milestone 4 added the line that names the selected file watcher. The tests run for milestone 4 did not include it. The golden file now has the line.
- `yak killall` sends `KILL` to every yak process, the forkserver included, so the forkserver's drops never run.
- The build report renders every error of an executor stage as an internal error (`app/yak_event_observer/src/display.rs`), so the Remote Execution configuration error reads `Internal error (stage: remote_upload_error)`. The tech-debt tracker records it.
- `IgnoreSet::from_ignore_spec` always ignores `yak-out` in the root cell (`app/yak_common/src/ignores/ignore_set.rs`), so both watchers drop its events after they arrive. Watchman still watches `yak-out` unless a `.watchmanconfig` lists it in `ignore_dirs`.

## Decision Log

- 2026-10-02: The notify watcher writes its sync marker in the version control directory of the project root (`.git`, `.hg`, `.jj`, or `.sl`) when one exists, and in the project root otherwise, as Watchman places its cookies. The marker stays out of `git status`, and the FSEvents stream does not leave out either directory.
- 2026-10-02: A sync marker that does not arrive within its timeout counts as dropped events. The watcher then rescans the project through `rescan.rs` and reports the reason in `incomplete_events_reason`, so a slow or lost marker cannot make a command read old contents.
- 2026-10-02: `yak.file_watcher` defaults to `auto`, which selects Watchman when `WATCHMAN_SOCK` is set or `watchman` is on the daemon's `PATH`, and the notify watcher otherwise. An installed Watchman that fails fails the command and names `yak.file_watcher = notify` in the error. A failure of an installed Watchman would otherwise switch the project to a different watcher without a report.
- 2026-10-02: The integration test harness keeps setting `yak.file_watcher = fs_hash_crawler` for every test project, so the suite runs the same watcher on every machine. The tests of `tests/core/io/` select their watcher in their `.yakconfig`, and `test_auto_file_watcher.py` selects `auto`.
- 2026-10-02: The Watchman watcher sends its query in `start_sync` and the sync processes the result, as the notify watcher writes its marker in `start_sync`. The query's clock advances only in a sync, so a later prefetch returns every event of an earlier one, and a failed prefetch makes the sync reconnect. With the prefetch, a no-op build took a median of 46 to 48 ms and the rewrite followed by `yak targets` took 83 ms with either watcher.
- 2026-10-02: The notify watcher keeps its snapshot current from its events, which the prototype of milestone 1 chose over a prefix invalidation in DICE. It changes only `yak_file_watcher`, and the snapshot already held the project in memory for rescans. A failed update or crawl leaves the snapshot lost, and the next sync crawls again and clears the DICE graph, because without a snapshot the paths under a changed directory are unknown.
- 2026-10-02: `FileWatcher::start_sync` writes the marker at the start of the command's DICE update, and `sync` waits for it after the configuration is loaded, so the FSEvents delay overlaps the configuration loading. The marker is written after the command started, which keeps the guarantee. A later `start_sync` replaces the marker and deletes its file, and a `sync` without a started marker writes its own. Milestone 9 shortens the configuration loading, so the overlap can shrink, and the measurement of milestone 9 covers the no-op build with the marker.
- 2026-10-02: A new watcher deletes markers older than 60 seconds, which a daemon that stopped during a sync left behind. It ignores the markers of other daemons, which watch the same project from other isolation directories.
- 2026-10-02: `ProcessGroupImpl` kills its process group when it is dropped before its leader was reaped. That covers the forkserver's exit after the daemon's socket closes, so the forkserver keeps no separate record of its process groups (item 2 of milestone 5). The daemon needs no cancellation of its commands at shutdown (item 3), because its exit closes the socket.
- 2026-10-02: `yak killall` kills the process group that each direct child of a killed yak process leads, before it kills that process. Once the parent dies, the actions belong to init and nothing can find them. A child that does not lead its own group is left alone, which keeps the kill to the groups that yak starts for actions.
- 2026-10-02: `yak killall` kills the yak processes of the current repository by default, and `--global` (`-g`) kills those of every repository, at the owner's request. `--repo` is gone because it is the default.
- 2026-10-02: Milestone 3 lands before milestone 2, because the prototype of milestone 1 was its implementation.
- 2026-10-02: Remote Execution keeps its retries for a backend that refuses connections, because they cover a backend that is restarting. A configuration without an engine or CAS address fails without retries.
- 2026-10-02: `REClientBuilder::configure` reads the addresses, TLS files, and headers once, and `new_retry` retries only `REClientBuilder::connect`, which fetches the capabilities. A configuration error then fails before the first attempt, whatever its label, so `with_error_handler` keeps labeling every error of the client builder (item 2).

## Outcomes & Retrospective

Milestones 1 to 7 are done. A renamed, removed, or replaced directory invalidates the paths under it with the notify watcher. Each command waits for the event of a sync marker, so it sees every change made before it started. `yak.file_watcher` defaults to `auto`, which selects Watchman when it is installed, and both watchers take the same time per command. `yak kill`, `yak killall`, and a killed daemon stop the local actions and the processes they started, and the tech-debt tracker lists the cases that still leave processes running. Milestone 6 needed only the shorter sleep in `test_init_data_timeout`, because a failed daemon start already returned within about 1 second. Milestone 7 is done: a command that needs Remote Execution without a configured address fails at once, which took the macOS integration suite from 1169 to 937 seconds.

## Context and Orientation

The file watcher tells DICE which files changed before each command. `yak_file_watcher` (`app/yak_file_watcher/src/`) holds the watchers, and `file_watcher.rs` selects one from `yak.file_watcher` (`watchman`, `notify`, or `fs_hash_crawler`, default `notify`).

- `notify.rs` collects events in `NotifyFileData` and turns them into invalidations in `sync`. `sync2` took the events that had arrived when the command started, which was the race of milestone 2.
- On macOS, `fsevents.rs` runs an FSEvents stream of the project root that leaves out `yak-out`, with latency 0 and `kFSEventStreamCreateFlagNoDefer`. Elsewhere, `notify::recommended_watcher` watches the root recursively.
- `rescan.rs` crawls the project into a `Snapshot` and compares two snapshots after the operating system drops events. The watcher takes the snapshot at its first sync and replaces it only after dropped events.
- `FileChangeTracker` (`app/yak_common/src/file_ops/dice.rs`) turns a changed path into DICE invalidations. `file_added_or_removed` and `dir_added_or_removed` invalidate the path's `ReadFileKey`, `PathMetadataKey`, and `ExistsMatchingExactCaseKey`, and the listings of its parent. Nothing invalidates the keys of paths under a renamed or removed directory, so a query of `//pkg/...` after `mv pkg pkg2` reads the cached listing and build file.
- `DiceKeyIndex` (`dice/dice/src/key_index.rs`) interns every key that DICE has seen, which a prefix invalidation could enumerate.

Local actions run through the forkserver on macOS and Linux (`yak.forkserver`, `app/yak_server/src/daemon/forkserver.rs`), which runs `spawn_command_and_stream_events` (`app/yak_execute_local/src/lib.rs`). Each action gets its own process group (`app/yak_execute_local/src/unix/process_group.rs`). The group is killed only when the action is cancelled or times out while its future is polled. `yak kill` (`app/yak_client_ctx/src/daemon/client/kill.rs`) asks the daemon to shut down, waits up to 4 seconds, and sends `SIGKILL` to the daemon's pid. The forkserver exits on EOF from the daemon and drops its streams, and the actions' process groups are reparented.

The client starts the daemon in `start_new_yakd_and_connect` (`app/yak_client_ctx/src/daemon/client/connect.rs`). `yak daemon` double-forks, so the client holds no handle to the daemon. The client retries a connection until the deadline of `YAKD_STARTUP_INIT_TIMEOUT` (default 90 seconds, 120 in the test harness). A daemon whose `DaemonState::new` fails writes `yakd_error_log` and exits. The retry loop reads neither that file nor the daemon's pid, and the client reads the error log only after the deadline.

`ReConnectionManager::new` (`app/yak_server/src/daemon/state.rs`) allows 10 connection attempts, and `new_retry` (`app/yak_execute/src/re/client.rs`) sleeps 1 to 9 seconds between them. `with_error_handler` (`app/yak_execute/src/re/error.rs`) labels every error of `build_and_connect` as a `RemoteExecutionError`, so `"No engine or CAS address"` (`remote_execution/re_grpc/src/client.rs`) is retried for 45 seconds. Test projects whose platforms set `remote_enabled = True` without `YAK_TEST_RE_CONFIG` hit it. The connection is lazy and cached per handle, so each command can pay it again.

`app/yak_server/src/tool_identity.rs` runs `rustc -vV`, `go version`, and `clang --version` in parallel in the project root on every command and hashes their output into `tool_identity.*` computed configuration values (`docs/exec-plans/completed/2026-10-01-key-actions-on-their-tools.md`). `load_new_configs` (`app/yak_server/src/ctx.rs`) calls it while the command holds the DICE update. `rustc` on `PATH` is often a rustup proxy, whose output depends on `rust-toolchain.toml`, `$RUSTUP_HOME/settings.toml`, `RUSTUP_TOOLCHAIN`, and the installed toolchains, while the proxy file stays the same.

## Plan of Work

### Milestone 1 (prototype): invalidation under a directory

Two designs can invalidate the paths under a renamed or removed directory.

- The watcher keeps its `Snapshot` current by applying each event, re-reading the metadata of the reported path and crawling a reported directory. A removed or renamed directory then reports each path the snapshot held under it, and a created one reports each path a crawl finds. The change stays inside `yak_file_watcher`, but the snapshot holds the whole project in memory from the first sync, as it does now.
- `DiceTransactionUpdater` gains an invalidation of every key of a type whose path starts with a prefix, through `DiceKeyIndex`. It covers keys of paths the watcher never saw, but it changes DICE's API and costs a scan of the index for each directory event.

The prototype implements the snapshot design behind the existing `rescan.rs` types and runs the reproduction of the tracker entry (`mv pkg pkg2`, then `yak uquery //pkg/...`) on macOS and Linux. It keeps the snapshot design if the query fails after the rename on both systems and a no-op `yak build` in this repository with the notify watcher stays within 5% of its time before the change. Otherwise it records the measurement and moves to the DICE design.

### Milestone 2: sync marker

1. `NotifyFileWatcher` (`app/yak_file_watcher/src/notify.rs`) creates a marker file named for the daemon's pid and a counter when the command's DICE update starts. `sync2` waits until the event handler sees an event for it, deletes it, and then takes the events. The handler records marker events apart from other events and never invalidates them.
2. A marker that does not arrive within 10 seconds, or that cannot be written, sets `missed_events`, so the sync rescans.
3. A unit test in `notify.rs` creates a file in a temporary project and syncs at once, 200 times, and checks that every sync reports its file. It fails before the change on macOS.
4. `tests/core/build/test_modify.py::test_modify_genrule_notify` and the loop of the tracker entry pass without misses.

### Milestone 3: invalidation under a directory

1. The chosen design from milestone 1, with unit tests for a rename, a removal, a rename back, and a nested package.
2. A test in `tests/core/io/test_notify.py` runs `yak uquery //pkg/...`, renames `pkg`, and checks that the query fails and that the renamed package answers at its new path. The same test runs for Watchman in `tests/core/io/test_watchman.py`.
3. If Watchman reports only the directory for a rename, the fix covers its watcher too.

### Milestone 4: Watchman when installed

1. `file_watcher.rs` accepts `auto` and makes it the default. `auto` selects Watchman when `WATCHMAN_SOCK` is set or `watchman` is on the daemon's `PATH`, and the notify watcher otherwise. The daemon logs which watcher it selected.
2. The Watchman query leaves out `yak-out` with a `dirname` expression, so its results do not carry build outputs.
3. `yak init` writes a `.watchmanconfig` with `ignore_dirs` of `yak-out`, so Watchman does not watch the outputs. The repository gets the same file.
4. `tests/e2e_util/yak_workspace.py` keeps its `yak.file_watcher` for every test project. New tests check that `auto` selects Watchman when it is on `PATH` and the notify watcher when it is not.
5. Measure a no-op `yak build` and a one-file edit in this repository with each watcher, and record the numbers here.
6. Documentation: a `[yak]` section in `website/docs/concepts/yakconfig.md` for `file_watcher` and the `.watchmanconfig` of a project that uses Watchman, and `CHANGELOG.md`. The page documents only `[alias]` and `[cells]` now.

### Milestone 5: local actions stop with the daemon

1. `ProcessGroupImpl` (`app/yak_execute_local/src/unix/process_group.rs`) kills its process group on drop when the child has not been reaped.
2. The forkserver records the process groups of its running commands and kills them before it exits after EOF. This covers a daemon that `SIGKILL` stopped, whose drops never run. Confirm whether the forkserver exits through `exit` or `_exit`, which decides whether the drops of item 1 run there.
3. On shutdown, the daemon cancels the running commands so the existing graceful kill runs within the shutdown deadline.
4. A test in `tests/core/daemon/` starts a build whose action writes its pid and the pid of a child to files and sleeps, waits for the files, runs `yak kill`, and checks that both processes are gone within 10 seconds.

### Milestone 6: failed daemon start

1. The connect retry of `start_new_yakd_and_connect` stops when `yakd_error_log` exists and returns its error, and stops when the pid of `yakd.pid` no longer runs.
2. `test_daemon_startup_error` (`tests/core/build/test_error_categorization.py`) keeps its golden output and finishes in less than 10 seconds.
3. Measure `test_init_data_timeout`. It waits for a timeout on purpose, with a daemon that sleeps 120 seconds before serving. If the teardown's `yak clean` waits on that daemon, shorten the sleep in the test.

### Milestone 7: missing Remote Execution address

1. A configuration without an engine or CAS address fails at the start of `new_retry` (`app/yak_execute/src/re/client.rs`) without retries, with an error that names `[yak_re_client]`.
2. `with_error_handler` labels only errors of the Remote Execution client as `RemoteExecutionError`, so configuration and TLS file errors are not retried.
3. A test checks that a command with `remote_enabled = True` and no address fails in less than 5 seconds. `tests/core/executor/test_remote_execution.py` keeps passing.
4. Record the time of the integration suite before and after.

### Milestone 8 (prototype): tool identity cache

The prototype keys a cache of each identity on file metadata (device, inode, size, modification time, and status change time):

- the tool's file on `PATH` and its canonical target;
- for `rustc`, `rust-toolchain.toml` and `rust-toolchain` in the project root and each ancestor, `$RUSTUP_HOME/settings.toml`, and the toolchain directory that `rustc --print sysroot` named at the last miss;
- for `go`, `go.mod` and `go.work` in the project root;
- for `clang` on macOS, the target of `/var/db/xcode_select_link`.

`PATH`, `RUSTUP_TOOLCHAIN`, and `DEVELOPER_DIR` come from the daemon's environment, which stays fixed for its lifetime.

It keeps the cache if each of these changes the identity at the next command:

- an edit of `rust-toolchain.toml`;
- `rustup default` and `rustup override set`;
- `rustup update` of the selected toolchain;
- a replacement of the tool's file on `PATH`;
- a Homebrew-style switch of a symlink's target;
- an edit of the `toolchain` line of `go.mod`;
- `xcode-select --switch`.

Otherwise, the plan records which change it missed and milestone 9 skips the cache.

### Milestone 9: commands skip unchanged tools

1. If the prototype kept the cache, `tool_identity.rs` keeps it on `RepoState` (`app/yak_server/src/daemon/state.rs`) and reruns only the tools whose key changed.
2. Either way, the identities run concurrently with the configuration parse, and a tool that a `-c tool_identity.<tool>` flag pins does not run.
3. Unit tests with a fake tool on a temporary `PATH` count its runs. A hit runs nothing, and a replaced or retargeted tool runs again. `tests/core/prelude/test_tool_identity.py` keeps passing.
4. Measure a no-op `yak build` against the 34 ms median of the tracker entry.

### Every milestone

Each milestone removes its tech-debt entries, updates the documentation it affects and `ARCHITECTURE.md` where a boundary or invariant changes, and is committed to `main` on its own.

## Validation and Acceptance

From the repository root:

- `python3 test.py yak_file_watcher yak_common` passes after milestones 2 and 3. `yak_common` reaches many crates, so run `python3 test.py` without packages after milestone 3.
- `python3 test.py yak_execute_local yak_forkserver yak_server` passes after milestone 5, `python3 test.py yak_client_ctx` after milestone 6, `python3 test.py yak_execute` after milestone 7, and `python3 test.py yak_server` after milestone 9.
- `tests/.venv/bin/python -m pytest tests/core/io tests/core/daemon tests/core/build/test_modify.py tests/core/build/test_error_categorization.py tests/core/executor tests/core/prelude/test_tool_identity.py -n auto` passes.
- `tests/.venv/bin/python -m pytest tests -n auto` passes on macOS at the end, with the same results as before apart from the fixed tests.
- The reproductions of the 6 tech-debt entries give their expected results:
  - the race loop misses no change;
  - the query after `mv pkg pkg2` fails;
  - `yak kill` leaves no `python3` process from the test actions;
  - `test_daemon_startup_error` finishes in less than 10 seconds;
  - a test with `remote_enabled = True` and no backend fails in less than 5 seconds;
  - a second no-op `yak build` runs no identity command.

## Idempotence and Recovery

Each milestone is a separate commit, so `git revert` of one leaves the others in place. Milestone 4 changes the default watcher, and `yak.file_watcher = notify` in `.yakconfig` restores the previous behavior. The sync marker is deleted after each sync. A marker that a killed daemon leaves behind has the daemon's pid in its name. The watcher ignores the events of other daemons' markers, and a new watcher deletes the markers older than 60 seconds.
