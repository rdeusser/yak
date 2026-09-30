# Test the targets a change affects

Follows [the plan contract](../../PLANS.md).

## Purpose

`yak test --changed-since <revision> [<pattern>...]` runs the tests whose results a change can affect, and skips the rest. The revision is anything Git resolves to a commit, such as `main`, `origin/main`, a branch, a tag, or a commit ID. The change is the difference between the working tree, including uncommitted and untracked files, and the merge base of the revision and `HEAD`. `git diff <revision>...` starts from the same merge base, so commits that reached the revision after the branch point do not count.

The repository owner's requirement (2026-09-30): the selection never skips a test that the change can affect. Speed comes second, and a close second. A selection that is too large costs time. A selection that misses a test is a defect.

To see it working, change one crate of a Cargo workspace and run `yak test --changed-since main //...`. It runs the tests of that crate and of the crates that depend on it, and no others.

## Progress

- [x] The owner chooses a flag on `yak test` that takes any Git revision (2026-09-30).
- [x] The owner chooses declared, enforced run-time files for tests (2026-09-30).
- [x] Milestone 1, prototype: dropped in favor of integration tests for each case in Validation (Decision Log).
- [x] Milestone 2: `yak test --changed-since`, selecting from the current graph (`app/yak_test/src/changed_since.rs`, `app/yak_client/src/commands/changed_since.rs`, `tests/core/test/test_changed_since.py`).
- [x] Milestone 3: member tests read only their package's files and the files that `[package.metadata.yak] test-data` declares, and run from their package's directory (`run_from_manifest_dir`, `ExternalRunnerTestInfo.working_directory`). `tests/core/generate/test_generate.py` covers a declared file and the failure after its declaration is removed (2026-09-30).
- [x] Milestone 4: documentation and validation against Roost. `yak test //...` in Roost passed 8 of 8 targets after Roost declared its test data, and `--changed-since HEAD` selected 28 of 28 matched targets with its `Cargo.toml` files edited (2026-09-30).
- [ ] Milestone 5: evaluate the changed packages at the merge base and compare their targets, so that a changed package selects only the targets that differ.

## Surprises & Discoveries

- A target hash (`app/yak_cmd_targets_server/src/target_hash.rs`, `ConfiguredTargetNode::target_hash` in `app/yak_node/src/nodes/configured.rs`) covers the target's label, the name of its rule, its attribute values, the contents of its input files, and the hashes of its dependencies. It does not cover the rule's implementation. A change to the `.bzl` file that defines a rule changes no hash, so a selection by hash alone skips every target of that rule.
- buck2-change-detector (`github.com/facebookincubator/buck2-change-detector`, read 2026-09-30) solves this for Buck2 CI. Its README states the aim: build and test only the targets a change can affect, and still detect every problem the change introduces. Its `btd` binary takes a list of changed files with their status (`git diff --name-status`, with renames split into a removal and an addition, or Sapling's status) and a dump of the base revision's targets. It takes a dump of the changed revision too, or with `--universe` it runs `buck2` itself. The dumps come from `buck2 targets --streaming --keep-going --no-cache --show-unconfigured-target-hash --json-lines --imports` with selected attributes, one from the base revision and one from the change (`td_util/src/buck/run.rs`). `immediate_target_changes` in `btd/src/diff.rs` marks a target as changed when it is new, its unconfigured hash differs, one of its inputs is a changed file, its package is a changed directory, one of its `ci_srcs` globs matches a changed file, or its rule's `.bzl` file changed directly or through the load graph from `--imports`. It then selects the reverse dependencies of the changed targets. A changed `.buckconfig` selects every target, behind an option. Changes to prelude `.bzl` files are ignored unless an option asks for them, `should_exclude_bzl_file_from_rule_impact` exempts named macros, and a depth limit on reverse dependencies is available. Each of those options trades correctness for a smaller selection. `ci_srcs` is how a target names files it reads that no attribute declares, and nothing checks the list.
- `btd` does not load the whole graph twice (`btd/src/rerun.rs`). Starting from the base revision's dump and the changes, it re-evaluates only the packages a change can alter: those whose build file changed, whose loaded `.bzl` files changed directly or through their loads, that a changed `PACKAGE` file covers, whose globs can match an added or removed file, and whose build file appeared or disappeared. Every other package evaluates as it did at the base revision, so its part of the base dump stands. A changed `.buckconfig` re-evaluates everything. The README adds that a dump can be reused across revisions while no build file, `PACKAGE` file, `.bzl` file, or configuration file changed, and a CI keeps the dump of each commit of its main branch for that reason. A change that touches only sources therefore needs one graph and a check of which targets' inputs changed.
- `yak targets` keeps every flag that `btd`'s dumps use (`--streaming`, `--keep-going`, `--no-cache`, `--show-unconfigured-target-hash`, `--json-lines`, `--imports`, `--package-values-regex`).
- Member tests of a Cargo workspace set `manifest_dir_in_project` (`prelude/decls/rust_rules.bzl`), so they can read any file of the workspace at run time through `CARGO_MANIFEST_DIR`, and no attribute declares those reads. Roost's `roost-unittest` runs `../roost-terminal/testdata/sh`, reads `../../rscript/api.ts`, and runs Git in its package's directory. A selection that trusts declared inputs would skip it after a change to `testdata/sh`.

- The unconfigured graph names targets that do not exist, such as `toolchains//:cxx_no_default_deps`, which a prelude attribute names by default and no configured target reaches. Counting every missing dependency as changed selected 15 of the 16 targets of `tests/core/generate/test_generate_data/workspace` with no change at all.
- A Cargo workspace that `yak generate` sets up defines every member's targets in the root package. A file that appears or disappears anywhere in the workspace, or a change to a `Cargo.toml`, marks that package, so it selects every test of the workspace. An edit to an existing source selects only the targets that have it as an input and their dependents: in `workspace`, an edit to `app/src/main.rs` selected `app` and its test, 2 of 16 targets.
- The runtime `CARGO_MANIFEST_DIR` of a `rust_test` was the plain value of `env`, which is relative to the crate's sources and does not resolve from the project root.

## Decision Log

- 2026-09-30: The flag takes a Git revision and compares the working tree with the merge base of the revision and `HEAD` (owner). Git resolves the revision with `git rev-parse --verify <revision>^{commit}`.
- 2026-09-30: The selection follows `btd`'s rules without the options that shrink it: a changed `.yakconfig` selects every test, prelude `.bzl` changes count, no `.bzl` file is exempt, and reverse dependencies have no depth limit. Unconfigured hashes compare the graph with every `select()` branch, so a dependency that one platform adds still counts.
- 2026-09-30: A test declares the files it reads at run time, and yak enforces the declaration (owner). An enforced declaration keeps the selection both correct and small. Treating every workspace file as an input of every member test is correct without changes to tests, but any change to a workspace would select all of its member tests. Unchecked declarations, like `ci_srcs`, keep the selection small, but a missing entry skips a test silently.
- 2026-09-30: A member lists the files its tests read in `[package.metadata.yak] test-data`, as glob patterns relative to its directory, like `include`. The files join the sources of the member's tests, so their changes select the tests. `CARGO_MANIFEST_DIR` of a member test names the member's directory in the compile action's symlinked sources, which hold the member's files and the declared files at their workspace paths, and `manifest_dir_in_project` goes away. The test runs from that directory, as Cargo runs a test from its package's directory, with the symlinked sources as an input of the test command. A read of an undeclared file through `CARGO_MANIFEST_DIR` or a relative path then fails, and so does running Git there, because the tree is no checkout. A read through an absolute path into the project is not enforced, which needs a sandbox.

- 2026-09-30: The selection reads the current graph only, and needs no second daemon. Evaluating a package depends only on the files it reads (its build file, the `PACKAGE` files above it, the files it loads, the listing of its directory, the configuration, and the external cells it reads), so a package whose inputs did not change evaluates as it did at the merge base, including its errors. A package whose inputs changed counts as changed in full, which costs precision (Surprises & Discoveries). Milestone 5 recovers it.
- 2026-09-30: The client parses the configuration of the merge base from Git and compares it with the current one, instead of listing the configuration files. A list misses a file that an include names, and an ignored, untracked file such as `.yakconfig.local` belongs to the machine and is read from disk for both.
- 2026-09-30: A missing target counts as changed only when its package changed, and a directory without a build file counts as changed only when a change lies under it. Evaluation is deterministic, so the target was missing at the merge base too otherwise.
- 2026-09-30: A changed package that defines a configuration target selects every test. Transitions and modifiers refer to configuration targets without a dependency edge, so no traversal reaches them.
- 2026-09-30: A matched target's target platform, from `--target-platforms` or `[parser] target_platform_detector_spec`, and the execution platforms of `[build] execution_platforms` are dependencies of the target for the selection.
- 2026-09-30: `rust-toolchain`, `rust-toolchain.toml`, and the paths in `[test] changed_since_select_all` select every test. rustup picks the compiler of every Rust action from the first two, and no action declares them.
- 2026-09-30: `test-data` files are sources of the member's tests only. As `include` files, they would also be inputs of the member's library, so a change to one would select the tests of every package that depends on it.
- 2026-09-30: `ExternalRunnerTestInfo.working_directory` sets a test's working directory, and `rust_test` with `run_from_manifest_dir` runs from its manifest directory in the crate's copy of its sources, with absolute paths. A launcher script could change directory without a provider field, but it adds a process to every test and needs a shell on each platform.
- 2026-09-30: The prototype of milestone 1 is dropped. Integration tests exercise each case of Validation against the implementation.

## Outcomes & Retrospective

Nothing yet.

## Context and Orientation

- `app/yak_client/src/commands/test.rs` parses `yak test` arguments, and `app/yak_server_commands` and `app/yak_test` run the tests.
- `app/yak_cmd_targets_server/src/target_hash.rs` computes target hashes, and `yak targets --imports` reports the load graph of each package.
- `rdeps()` and `owner()` in `app/yak_query/src/query/syntax/simple/functions.rs` are the query forms of reverse dependencies and file owners.
- `.yakconfig`, `.yakconfig.local`, and `.yakconfig.d/` are the project configuration (`website/docs/concepts/yakconfig.md`). The prelude is part of the `yak` binary, so it is the same at both revisions unless the project defines its own prelude cell.
- The `cargo` external cell (`app/yak_external_cells_cargo`) derives targets from `Cargo.toml` and `Cargo.lock`, so a change to either changes the targets of the `crates` cell, which a dump of the base revision captures.

## Plan of Work

1. The client (`app/yak_client/src/commands/changed_since.rs`) resolves the revision and the merge base with Git, lists the changed paths with `git diff --raw -z --no-renames --relative <merge base>` and `git ls-files --others --exclude-standard -z`, drops `yak-out`, and compares the configuration of the merge base with the current one (`YakConfigBasedCells::describe_difference`). It sends them as `TestRequest.changed_since`.
2. The daemon (`app/yak_test/src/changed_since.rs`) evaluates every package of the cells in the project, marks the packages whose evaluation can differ, and walks the dependencies of the matched targets, including configuration dependencies and platforms. It returns every target when a rule of Decision Log selects every test.
3. The test driver (`app/yak_test/src/command.rs`) tests only the selected targets, and prints how many it tests or why it tests all of them.
4. Member tests of a Cargo workspace take their `test-data` files as sources and run with `run_from_manifest_dir` (`app/yak_external_cells_cargo/src/workspace.rs`, `prelude/rust/rust_binary.bzl`), which sets `ExternalRunnerTestInfo.working_directory` (`app/yak_build_api`, `app/yak_test/src/orchestrator.rs`). Roost lists its tests' files in `crates/roost/Cargo.toml` and `crates/roost-script/Cargo.toml`.

Milestone 5 evaluates the changed packages in a `git worktree` of the merge base, with a daemon in its own isolation directory, and compares the unconfigured hashes of their targets with the current ones. A target whose hash and rule are unchanged is not changed. No proposal yet for reusing that evaluation across runs.

## Validation and Acceptance

Each case runs in a project under `tests/`, with the expected selection:

- A change to a library's source selects the library's tests and the tests of every target that depends on it, and no other test.
- A change to a `.bzl` file that defines a rule selects the tests of every target of that rule, with no attribute changed.
- A change to a macro that changes a target's attributes selects that target's tests.
- A new file that a `glob` matches selects the tests of the target whose `srcs` gain it.
- A change to `.yakconfig` selects every test.
- A change to `Cargo.lock` that moves a dependency to another version selects the tests of every crate that depends on it.
- A change to a file that a test reads at run time selects that test, under the Decision Log's rule for such files.
- A change to a file that no target reads selects no test.

`yak test --changed-since HEAD` with no change runs no test.

## Idempotence and Recovery

`--changed-since` writes nothing into the repository and reads Git only through commands that leave the index and the working tree unchanged. A run that fails can be repeated as is. Milestone 5 adds a worktree of the merge base under `yak-out`, which a run recreates when it finds the worktree at another commit and which `yak clean` removes.
