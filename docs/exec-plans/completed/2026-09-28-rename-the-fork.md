# Rename the fork to yak

Follows [the plan contract](../../PLANS.md).

## Purpose

The fork is an internal tool of the owner's company named yak.
After the change, the repository names the tool, its files, its crates, its configuration, and its values yak. The upstream tool's name appears only where text refers to the upstream project or to the history of the fork.

## Progress

- [x] The owner picks the name yak and the build file name `YAK` (2026-09-28).
- [x] The binary, its files and directories, the environment variables, and the configuration sections take yak names (2026-09-28).
- [x] The documentation, the website, and the messages and comments of the code call the tool yak (2026-09-29).
- [x] The Cargo packages and their directories take yak names (2026-09-29).
- [x] Every remaining occurrence of the upstream name in file contents and paths takes the yak form, and the third-party generators named after the upstream tool are removed (2026-09-29).
- [x] Links to the upstream project, the upstream names in `CHANGELOG.md`, and the history in `docs/exec-plans/` take their upstream names again (2026-09-29).

## Surprises & Discoveries

- On a case-insensitive file system, a directory named `yak` cannot sit beside a build file named `YAK`. The worker protocol of `examples/persistent_worker` lives in `proto/yak_worker/` for that reason.
- Two event logs in `tests/core/console/fixtures/` and the checked-in `xcode_version_checker` binary held the upstream name in binary data. The event logs were rewritten through a protobuf round trip that reproduced the original bytes before any string changed. The binary was rebuilt from its source with its `Makefile`.

## Decision Log

- 2026-09-28: The binary is `yak`, it reads `YAK` build files and `.yakconfig` files, and it writes `yak-out`. It reads none of the upstream names.
- 2026-09-29: The owner directed that nothing in the repository keeps the upstream name, with no compatibility names, and that the rename does not wait for tests. Values that other programs read, the protocol buffer packages, and the query attributes take the yak form as well.
- 2026-09-29: Text that refers to the upstream project or to the fork's history keeps the upstream names (owner). It covers links to `github.com/facebook/buck2` and to the Buck1 repository and site (`github.com/facebook/buck`, `buck.build`), the statements that the repository is a fork of `facebook/buck2`, Buck1, the upstream names of what `CHANGELOG.md` lists as removed, and the progress and decision records of the plans in `docs/exec-plans/`.
- 2026-09-29: The generator of third-party Rust rules, its configuration and fixups in `third-party/rust/`, its download script in `bootstrap/`, its CI action, and the Go generator in `prelude/go/tools/` are removed. The `YAK` files of the repository still name `//third-party/rust:<crate>`, so the yak build of the repository does not load (tech-debt tracker).
- 2026-09-29: The prelude's helpers for generated third-party Rust packages keep their behavior under Cargo names, such as `get_cargo_platforms` and the package value `rust.cargo_platforms`.
- 2026-09-29: `prelude/ide_integrations/visual_studio/msvs/absolutize_path.exe`, a Meta binary without source, is removed. The generated Visual Studio projects run yak without it.

## Outcomes & Retrospective

The rename landed in the commits from `04fcc47b52` to the commit that completes this plan. One commit replaced the remaining occurrences in file contents and paths in one pass and ran no tests or builds. That pass also mapped links to the upstream project, such as `github.com/facebook/buck2`, to `yak` names that do not exist. The commit after it restored the upstream names in those links, in `CHANGELOG.md`, and in the history of the plans in `docs/exec-plans/`, by comparing each line with the mapped text of its version before the pass. Links to the fork's GitHub repository and site use `yak` paths, which resolve only after the repository on GitHub is renamed to `yak`.

## Context and Orientation

The binary's names are defined in `app/yak/`, `app/yak_common/src/invocation_paths.rs`, `app/yak_common/src/legacy_configs/path.rs`, and `app/yak_common/src/buildfiles.rs`. `CHANGELOG.md` lists the user-visible changes.

## Plan of Work

The work is complete.

## Validation and Acceptance

From the repository root, `git grep -l -i -P 'b[u]ck(?!et)' -- ':!CHANGELOG.md' ':!docs/exec-plans'` lists only files that link to or name the upstream project, and `git ls-files | grep -i -P 'b[u]ck(?!et)'` lists only plans in `docs/exec-plans/completed/`.

## Idempotence and Recovery

The commits of the rename can be reverted in reverse order.
