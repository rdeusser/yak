# Test the targets a change affects

Follows [the plan contract](../../PLANS.md).

## Purpose

`yak test --changed-since <revision> [<pattern>...]` runs the tests whose results a change can affect, and skips the rest. The revision is anything Git resolves to a commit, such as `main`, `origin/main`, a branch, a tag, or a commit ID. The change is the difference between the working tree, including uncommitted and untracked files, and the merge base of the revision and `HEAD`. `git diff <revision>...` starts from the same merge base, so commits that reached the revision after the branch point do not count.

The repository owner's requirement (2026-09-30): the selection never skips a test that the change can affect. Speed comes second, and a close second. A selection that is too large costs time. A selection that misses a test is a defect.

To see it working, change one crate of a Cargo workspace and run `yak test --changed-since main //...`. It runs the tests of that crate and of the crates that depend on it, and no others.

## Progress

- [x] The owner chooses a flag on `yak test` that takes any Git revision (2026-09-30).
- [x] The owner chooses declared, enforced run-time files for tests (2026-09-30).
- [ ] Milestone 1, prototype: compute the selection from two `yak targets` dumps for the cases in Validation, and compare it with the tests that fail after each change.
- [ ] Milestone 2: `yak test --changed-since`.
- [ ] Milestone 3: member tests read only their package's files and the files that `[package.metadata.yak] test-data` declares, and run from their package's directory.
- [ ] Milestone 4: documentation and validation against Roost.

## Surprises & Discoveries

- A target hash (`app/yak_cmd_targets_server/src/target_hash.rs`, `ConfiguredTargetNode::target_hash` in `app/yak_node/src/nodes/configured.rs`) covers the target's label, the name of its rule, its attribute values, the contents of its input files, and the hashes of its dependencies. It does not cover the rule's implementation. A change to the `.bzl` file that defines a rule changes no hash, so a selection by hash alone skips every target of that rule.
- buck2-change-detector (`github.com/facebookincubator/buck2-change-detector`, read 2026-09-30) solves this for Buck2 CI. Its README states the aim: build and test only the targets a change can affect, and still detect every problem the change introduces. Its `btd` binary takes a list of changed files with their status (`git diff --name-status`, with renames split into a removal and an addition, or Sapling's status) and a dump of the base revision's targets. It takes a dump of the changed revision too, or with `--universe` it runs `buck2` itself. The dumps come from `buck2 targets --streaming --keep-going --no-cache --show-unconfigured-target-hash --json-lines --imports` with selected attributes, one from the base revision and one from the change (`td_util/src/buck/run.rs`). `immediate_target_changes` in `btd/src/diff.rs` marks a target as changed when it is new, its unconfigured hash differs, one of its inputs is a changed file, its package is a changed directory, one of its `ci_srcs` globs matches a changed file, or its rule's `.bzl` file changed directly or through the load graph from `--imports`. It then selects the reverse dependencies of the changed targets. A changed `.buckconfig` selects every target, behind an option. Changes to prelude `.bzl` files are ignored unless an option asks for them, `should_exclude_bzl_file_from_rule_impact` exempts named macros, and a depth limit on reverse dependencies is available. Each of those options trades correctness for a smaller selection. `ci_srcs` is how a target names files it reads that no attribute declares, and nothing checks the list.
- `btd` does not load the whole graph twice (`btd/src/rerun.rs`). Starting from the base revision's dump and the changes, it re-evaluates only the packages a change can alter: those whose build file changed, whose loaded `.bzl` files changed directly or through their loads, that a changed `PACKAGE` file covers, whose globs can match an added or removed file, and whose build file appeared or disappeared. Every other package evaluates as it did at the base revision, so its part of the base dump stands. A changed `.buckconfig` re-evaluates everything. The README adds that a dump can be reused across revisions while no build file, `PACKAGE` file, `.bzl` file, or configuration file changed, and a CI keeps the dump of each commit of its main branch for that reason. A change that touches only sources therefore needs one graph and a check of which targets' inputs changed.
- `yak targets` keeps every flag that `btd`'s dumps use (`--streaming`, `--keep-going`, `--no-cache`, `--show-unconfigured-target-hash`, `--json-lines`, `--imports`, `--package-values-regex`).
- Member tests of a Cargo workspace set `manifest_dir_in_project` (`prelude/decls/rust_rules.bzl`), so they can read any file of the workspace at run time through `CARGO_MANIFEST_DIR`, and no attribute declares those reads. Roost's `roost-unittest` runs `../roost-terminal/testdata/sh`, reads `../../rscript/api.ts`, and runs Git in its package's directory. A selection that trusts declared inputs would skip it after a change to `testdata/sh`.

## Decision Log

- 2026-09-30: The flag takes a Git revision and compares the working tree with the merge base of the revision and `HEAD` (owner). Git resolves the revision with `git rev-parse --verify <revision>^{commit}`.
- 2026-09-30: The selection follows `btd`'s rules without the options that shrink it: a changed `.yakconfig` selects every test, prelude `.bzl` changes count, no `.bzl` file is exempt, and reverse dependencies have no depth limit. Unconfigured hashes compare the graph with every `select()` branch, so a dependency that one platform adds still counts.
- 2026-09-30: A test declares the files it reads at run time, and yak enforces the declaration (owner). An enforced declaration keeps the selection both correct and small. Treating every workspace file as an input of every member test is correct without changes to tests, but any change to a workspace would select all of its member tests. Unchecked declarations, like `ci_srcs`, keep the selection small, but a missing entry skips a test silently.
- 2026-09-30: A member lists the files its tests read in `[package.metadata.yak] test-data`, as glob patterns relative to its directory, like `include`. The files join the sources of the member's tests, so their changes select the tests. `CARGO_MANIFEST_DIR` of a member test names the member's directory in the compile action's symlinked sources, which hold the member's files and the declared files at their workspace paths, and `manifest_dir_in_project` goes away. The test runs from that directory, as Cargo runs a test from its package's directory, with the symlinked sources as an input of the test command. A read of an undeclared file through `CARGO_MANIFEST_DIR` or a relative path then fails, and so does running Git there, because the tree is no checkout. A read through an absolute path into the project is not enforced, which needs a sandbox.

## Outcomes & Retrospective

Nothing yet.

## Context and Orientation

- `app/yak_client/src/commands/test.rs` parses `yak test` arguments, and `app/yak_server_commands` and `app/yak_test` run the tests.
- `app/yak_cmd_targets_server/src/target_hash.rs` computes target hashes, and `yak targets --imports` reports the load graph of each package.
- `rdeps()` and `owner()` in `app/yak_query/src/query/syntax/simple/functions.rs` are the query forms of reverse dependencies and file owners.
- `.yakconfig`, `.yakconfig.local`, and `.yakconfig.d/` are the project configuration (`website/docs/concepts/yakconfig.md`). The prelude is part of the `yak` binary, so it is the same at both revisions unless the project defines its own prelude cell.
- The `cargo` external cell (`app/yak_external_cells_cargo`) derives targets from `Cargo.toml` and `Cargo.lock`, so a change to either changes the targets of the `crates` cell, which a dump of the base revision captures.

## Plan of Work

1. Resolve the revision and the merge base with Git, and list the changed paths with `git diff --name-status --no-renames -z <merge base>` and `git ls-files --others --exclude-standard -z`. A rename counts as a deletion and an addition.
2. Build both graphs from as little evaluation as possible, as `btd/src/rerun.rs` does. The running daemon already holds the change's graph. If only sources changed, with no file added or removed, the two graphs differ only in the contents of inputs, and no second graph is needed. Otherwise, evaluate at the merge base only the packages the change can alter, from a `git worktree` of it with a daemon in its own isolation directory, and take every other package from the change's graph. A change to `.yakconfig`, `.yakconfig.local`, `.yakconfig.d/`, a file the configuration includes, or the files an external cell reads (`Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `go.mod`, `go.sum`, `go.work`) re-evaluates the affected cell or everything. Keep each merge base's evaluation in `yak-out`, keyed by its commit, so later runs against the same merge base reuse it.
3. Mark changed targets as `btd` does, with the Decision Log's settings, then select the test targets among the changed targets and their reverse dependencies.
4. Run the selected tests, as `yak test` runs a pattern's tests. Report how many tests the selection skipped, and why each selected test was selected under `--verbose`.
5. Add `test-data` to the member macro (`app/yak_external_cells_cargo/src/workspace.rs`), remove `manifest_dir_in_project` from `prelude/decls/rust_rules.bzl` and `prelude/rust/build.bzl`, and run `rust_test` from a directory that its target names. `ExternalRunnerTestInfo` has no working directory beyond `run_from_project_root`, so this needs a field or a launcher. Roost then lists `../roost-terminal/testdata/**` and `../../rscript/**` in `crates/roost/Cargo.toml`, and the test that runs Git in its package's directory needs a change in Roost.

No proposal yet for reusing the base revision's dump across runs, which CI would want for a busy `main`.

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

`--changed-since` writes nothing into the repository. The worktree of the merge base lives under `yak-out` and is recreated when a run finds it at another commit. A run that fails partway leaves the worktree and its daemon, which the next run reuses or `yak clean` removes.
