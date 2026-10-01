# Port the upstream commits since the fork

Follows [the plan contract](../../PLANS.md).

## Purpose

The fork takes the fixes, features, and dependency updates that `facebook/buck2` made after the fork point `903bfd7a61`, up to `35e4987f6f` (2026-09-30), 140 commits.
Each commit that the fork takes lands as one commit with `Ported from facebook/buck2@<hash>`, and each commit that it skips has a line in `tools/port_upstream/skipped.txt` with the reason.
Afterwards, `tools/port_upstream/port.py pending` lists nothing for that range, and `python3 test.py` and the integration tests pass.

## Progress

- [x] `tools/port_upstream/port.py` ports one commit or runs through the pending ones, with unit tests in `tools/port_upstream/test_port.py` (2026-09-30).
- [x] Port or skip the 140 commits in `903bfd7a61..35e4987f6f`, building and testing after each batch (2026-09-30). 115 commits landed as ports, and `tools/port_upstream/skipped.txt` lists the 25 skipped commits with their reasons.
- [x] Check every port with `port.py check` (2026-09-30). The lines it reports are present at `HEAD`, or they belong to code that the fork removed.
- [x] Run `python3 test.py` and the integration tests over the result (2026-09-30). `python3 test.py` passed for the whole workspace. `tests/.venv/bin/python -m pytest tests -n auto` gave 1827 passed, 208 skipped, and 3 expected failures on macOS.

## Surprises & Discoveries

- The fork point with yak names differs from `HEAD` in 3,329 files at the same path, and most of those differences are `use` lines that `cargo fmt` sorted again after the rename. A three-way merge of each file absorbs them.
- Git's rename detection pairs 860 files that the fork moved, such as `docs/` to `website/docs/`, and misses `prelude/toolchains/demo.bzl`, which became `prelude/toolchains/system.bzl` with larger changes. `MOVES` in `port.py` lists it.
- Updating one package with `cargo update --precise` can move another, such as `thiserror` moving `thiserror-impl`, so `port.py` reads the lock file again before each update.
- An update can also fail until another package has moved. `brotli` could not go back to 8.0.4 until `compression-codecs` had moved to 0.4.44, so `port.py` repeats the failed updates while any update succeeds.
- Rename detection paired deleted `shim/` files and stubs with small test fixtures that the fork added, such as `shim/build_defs/config.bzl` with the `test_changed_since` fixture's `rules/config.bzl`. `port.py` now rejects a pair with unrelated file names, or one that leaves a directory the fork mostly deleted for a directory of another name.
- The fork pruned `website/package-lock.json`, so `git merge-file` aligned its conflict blocks away from the fork's copy of the entries that upstream's version bumps change. `port.py` applies such edits to the whole file where the lines they replace occur once.
- The fork's own differences from upstream that ports run into:
  - The fork defaults to SHA256 digests and has no BLAKE3-KEYED, so upstream tests that assume SHA1 set it.
  - Tests that need Remote Execution carry `@pytest.mark.remote_execution`.
  - `SettingKey` has one default, and settings sections have no rollout metadata.
  - The Remote Execution client uses the protocol's platform, sends no Meta metadata, and records no costs or digest traces.
  - `tag_error!` has no `task` or `action_cache_is_corrupted` fields.
  - `CommonAttributeArgs::get` cannot fail, because the fork removed `--output-attributes`.
  - Fixtures that use `?modifier` need a `PACKAGE` file with `set_cfg_constructor`.
- The first full integration run failed 215 tests, because the port of `c2620f20ad` dropped its new file in `prelude/android` and kept the `load` of it in `prelude/rules_impl.bzl`, so the prelude did not load. The fork skips that commit, and `port.py` now stops for review when a port refers to a file that it dropped.
- A merge can bring back a test that the fork deleted, when upstream changes the lines next to it. The port of `67d957af84` brought back `test_cas_artifact`, which needs Meta's CAS.
- Upstream adds tests whose fixtures only Meta's repository has. `5cc030a739` added `tests/e2e/test_cxx_flags.py` for fixtures under `tests/targets`, so the fork tests `cxx_flags` in `tests/prelude/test_prelude_rules.py` instead.
- Ports can compile and still leave a fork incompatibility in a crate that a later port reaches first. `cargo check --workspace --tests` after each batch found `CommonAttributeArgs::get()?` in the ported `yak debug anon-targets` client.

## Decision Log

- 2026-09-30: The owner asked for the upstream commits to be ported with judgement about which apply, and for the port script to be as automatic as possible.
- 2026-09-30: `port.py` merges each file three ways. The merge base and the upstream side are the upstream file before and after the commit with yak names, and the fork's side is the file at `HEAD`. Upstream `BUCK` files differ too much from `YAK` files for a merge, so their dependency list changes are applied to the `YAK` file.
- 2026-09-30: `port.py` adapts upstream code to the fork's differences with rules in `REPLACEMENTS` when the difference is a fixed spelling (`re_platform(platform)`, the `use_fbcode_metadata` argument, the two `SettingKey` defaults, diff IDs in comments). Differences in the shape of the code stay manual and go in the port's `--note`.
- 2026-09-30: A port that adds a line for Meta's internal build, such as `#[cfg(fbcode_build)]`, stops for review. `1f3bcd7d4f` added only such a line to `rust-project`, and the fork skips it.
- 2026-09-30: A commit is skipped when it changes only code that the fork removed (the JVM, Android, Kotlin, and JavaScript rules, the Go generator, `shim/`, and the agent context that `CHANGELOG.md` lists as removed), or when it serves only Meta's internal build or tools (Dotslash updates and changes to Meta's internal interfaces). A commit that changes both kept and removed code is ported without the removed parts.

## Outcomes & Retrospective

The fork took 115 of the 140 commits and skipped 25.
The skipped commits change the agent context, the JVM, Android (including a native build commands sub-target), Kotlin and Go generator code, `shim/`, in-place test runs, or Meta's `rust-project` sysroot, or they revert each other.
Five ports carry a `Changes for the fork` note on their manual adaptation.
Follow-up commits name the ports whose fork incompatibilities showed up later, such as `Remove the Meta build leftovers of ported commits`.
Each manual resolution that a fixed rule could reproduce became a rule in `port.py`, with a test in `test_port.py`.
`tools/port_upstream/README.md` lists the fork differences that still need a manual adaptation, and the tech-debt tracker records that `cas_artifact` has no test.

## Context and Orientation

`docs/developers/basics.md` states the rules for a ported commit, and `tools/port_upstream/README.md` describes the tool.
The `upstream` remote names `https://github.com/facebook/buck2.git`, which `port.py fetch` adds.
`CHANGELOG.md` lists what the fork removed.

## Plan of Work

1. Commit the tool, its README, and this plan.
2. Run `tools/port_upstream/port.py run` from the repository root. At each stop, resolve the listed files against the fork's code, or skip the commit with `port.py abort` and `port.py skip <commit> <reason>`, then `port.py continue` or `port.py run` again.
3. After each batch of about 20 commits, run `cargo build --bin=yak` and `python3 test.py` for the packages the batch changed, and fix what the port broke in the commit that broke it where it is the latest, or in a follow-up commit that names it.
4. Commit `tools/port_upstream/skipped.txt` with the skips of each batch.

## Validation and Acceptance

From the repository root:

- `python3 tools/port_upstream/test_port.py` passes.
- `tools/port_upstream/port.py pending` prints only commits after `35e4987f6f`.
- `python3 test.py` passes, and `tests/.venv/bin/python -m pytest tests -n auto` passes.

## Idempotence and Recovery

`port.py abort` discards a port in progress and keeps `skipped.txt`.
`pending` reads the trailers in the history of `HEAD`, so a ported commit that `git revert` undoes still counts as ported. Remove the commit with `git rebase` to port it again.
