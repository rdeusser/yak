# Port the upstream commits since the fork

Follows [the plan contract](../../PLANS.md).

## Purpose

The fork takes the fixes, features, and dependency updates that `facebook/buck2` made after the fork point `903bfd7a61`, up to `35e4987f6f` (2026-09-30), 140 commits.
Each commit that the fork takes lands as one commit with `Ported from facebook/buck2@<hash>`, and each commit that it skips has a line in `tools/port_upstream/skipped.txt` with the reason.
Afterwards, `tools/port_upstream/port.py pending` lists nothing for that range, and `python3 test.py` and the integration tests pass.

## Progress

- [x] `tools/port_upstream/port.py` ports one commit or runs through the pending ones, with unit tests in `tools/port_upstream/test_port.py` (2026-09-30).
- [ ] Port or skip the 140 commits in `903bfd7a61..35e4987f6f`, building and testing after each batch.
- [ ] Run `python3 test.py` and the integration tests over the result.

## Surprises & Discoveries

- The fork point with yak names differs from `HEAD` in 3,329 files at the same path, and most of those differences are `use` lines that `cargo fmt` sorted again after the rename. A three-way merge of each file absorbs them.
- Git's rename detection pairs 860 files that the fork moved, such as `docs/` to `website/docs/`, and misses `prelude/toolchains/demo.bzl`, which became `prelude/toolchains/system.bzl` with larger changes. `MOVES` in `port.py` lists it.
- Updating one package with `cargo update --precise` can move another, such as `thiserror` moving `thiserror-impl`, so `port.py` reads the lock file again before each update.

## Decision Log

- 2026-09-30: The owner asked for the upstream commits to be ported with judgement about which apply, and for the port script to be as automatic as possible.
- 2026-09-30: `port.py` merges each file three ways. The merge base and the upstream side are the upstream file before and after the commit with yak names, and the fork's side is the file at `HEAD`. Upstream `BUCK` files differ too much from `YAK` files for a merge, so their dependency list changes are applied to the `YAK` file.
- 2026-09-30: A commit is skipped when it changes only code that the fork removed (the JVM, Android, Kotlin, and JavaScript rules, the Go generator, `shim/`, and the agent context that `CHANGELOG.md` lists as removed), or when it serves only Meta's internal build or tools (Dotslash updates and changes to Meta's internal interfaces). A commit that changes both kept and removed code is ported without the removed parts.

## Outcomes & Retrospective

Nothing yet.

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
