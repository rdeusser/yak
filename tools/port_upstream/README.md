# port_upstream

`port.py` ports commits of `facebook/buck2` to this repository, one commit here for each upstream commit.
It needs Python 3.10 or later, Git, and Cargo, and runs from any directory of the repository.

## Commands

| Command | Result |
| --- | --- |
| `port.py fetch` | Adds the `upstream` remote if it is missing and fetches its `main` branch. |
| `port.py pending` | Lists the upstream commits after the fork point that are neither ported nor skipped, oldest first. |
| `port.py run [--limit N]` | Ports the pending commits in order and stops at the first one that needs attention. |
| `port.py apply <commit> [--no-commit]` | Ports one commit. |
| `port.py continue [--note TEXT]` | Commits the port in progress after its files are resolved. It stages the files that the port wrote, and commits other changes only when they are staged. The note says how the port differs from upstream. |
| `port.py abort` | Resets the files that the port in progress wrote. |
| `port.py skip <commit> <reason>` | Records that the fork does not take a commit. |
| `port.py check <port>...` | Lists the lines that the upstream commit of each port adds and the port does not. |
| `port.py message <commit>` | Prints the commit message that a port of the commit gets. |

A ported commit keeps the upstream author and author date and ends with `Ported from facebook/buck2@<hash>`.
`pending` reads those trailers from `git log` and the skipped commits from `skipped.txt`.
`port.py` appends to `skipped.txt` and never commits it, so its changes are committed on their own.

## How a commit is ported

Each file that the upstream commit changes takes its path in this repository:

1. The upstream names in the path take the yak form (`app/buck2_core/BUCK` becomes `app/yak_core/YAK`).
2. A file that the fork moved takes its new path. Git's rename detection between the fork point with yak names and `HEAD` finds the moves, and `MOVES` in `port.py` lists the moves that it misses. A pair that rename detection finds counts as a move when the file names are related, and when the file keeps its directory's name, moves up into an ancestor, or leaves a directory that the fork kept. Other pairs, such as a deleted stub and a new test fixture of similar content, count as a deleted file.
3. A file that the fork deleted, or a new file in a directory that the fork deleted, is dropped.

The contents of the file before and after the upstream commit take the yak names.
`rename` lists the names and the text about the upstream project that keeps its names.
Lines that upstream marks `@oss-disable` are dropped, and lines that it marks `@oss-enable` are kept.
`git merge-file` applies the difference between the two to the file of this repository.
In Rust files, each run of one-line `use` items is sorted on all three sides first, because the yak names sort differently from the upstream names.
In `Cargo.toml` files, the upstream sides' dependency entries take the fork's order first, for the same reason.
`port.py` resolves three kinds of conflict block:

- The two sides change adjacent lines, and their edits do not overlap.
- The fork deleted the lines that upstream changed, and upstream added no more lines than it replaced. The output notes each such block.
- Each upstream edit replaces lines that occur once in the fork's side of the block, such as a version in a lock file that the fork pruned. The output notes each such block.

If blocks remain, upstream's edits apply to the whole file when the lines that each edit replaces occur once in it, or fall in lines that the fork deleted.
This resolves files that the fork pruned heavily, where `git merge-file` aligns the blocks away from the fork's copy of the changed lines.

A file that already holds every upstream edit, such as a golden file that the fork regenerated, stays as it is. A file that holds some of them is marked for review.

Build files take the labels that the upstream change adds to or removes from a list of a rule, and keep their lists sorted.
The crate's `Cargo.toml` takes the same changes to `deps` and `test_deps`, as `<name>.workspace = true` lines, because upstream generates its `Cargo.toml` files from its build files.
A build file change that does more is marked for review.

`Cargo.lock` files are not merged.
When the port finishes, each package that the upstream commit moves from one version to another moves the same way with `cargo update --precise`, and `cargo metadata` resolves the requirements of the ported `Cargo.toml` files.

`rustfmt` formats the ported Rust files when the port finishes.

A commit whose files the fork removed is recorded in `skipped.txt` without a commit.
A commit that needs attention stops with its files staged and lists them:

| Mark | Meaning |
| --- | --- |
| `CONFLICT` | The file has conflict markers, or upstream deleted a file that the fork changed. |
| `REVIEW` | Upstream changed a build file beyond its dependency lists, the fork already has some of upstream's edits, the port adds lines for Meta's internal build, such as `#[cfg(fbcode_build)]`, or the port adds a reference to a file that it dropped, such as a `load` of a file in `prelude/android`. |
| `PROBLEM` | A `cargo` command failed. |

The path map is cached in `.git/yak-port/` by the tree of `HEAD` and the source of `port.py`.

## Fork differences that ports meet

`REPLACEMENTS` in `port.py` handles the differences that have a fixed spelling.
These differences need a manual adaptation, recorded in the port's `--note`:

- The fork defaults to SHA-256 digests and has no BLAKE3-KEYED. Upstream tests that declare SHA-1 checksums set `YAK_DEFAULT_DIGEST_ALGORITHM` to `SHA1`.
- Tests that need Remote Execution carry `@pytest.mark.remote_execution`, and upstream tests of fixtures under `tests/targets` have no fixtures in the fork.
- The Remote Execution client records no costs or digest traces.
- `tag_error!` has no `task` or `action_cache_is_corrupted` fields.
- Settings sections have no rollout metadata.
- `CommonAttributeArgs::get` returns the attribute list without an error.
- Fixtures that use `?modifier` need a `PACKAGE` file with `set_cfg_constructor`.
- The fork has no proxy allowlist and no VPN-less URL for downloads.

## Tests

`python3 tools/port_upstream/test_port.py` runs the unit tests.
