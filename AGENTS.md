# Agent instructions

This repository is a fork of Meta's Buck2 build system.
It holds the Rust client and daemon (`app/`), the Starlark interpreter (`starlark-rust/`), the incremental computation engine (`dice/`), the Starlark rule library (`prelude/`), and the crates they use.
The fork is named yak. The binary is `yak`, it reads `YAK` build files and `.yakconfig` files, and it writes `yak-out`.
[The tech-debt tracker](docs/exec-plans/tech-debt-tracker.md) lists what still depends on the upstream project, such as release downloads.

## Read before changing code

- [ARCHITECTURE.md](ARCHITECTURE.md) maps the crates, the build pipeline, and the rules about which crates may depend on which.
- [docs/developers/basics.md](docs/developers/basics.md) covers building, validation, coding conventions, error handling, feature gates, and dependency changes.
- [docs/developers/.llms/code_quality.md](docs/developers/.llms/code_quality.md) and [docs/developers/.llms/writing.md](docs/developers/.llms/writing.md) set the expectations for code and for comments and documentation.
- [docs/developers/debugging.md](docs/developers/debugging.md) and [docs/developers/what-ran.md](docs/developers/what-ran.md) cover logs, tracing, and rerunning a build's commands.
- [docs/developers/perf/basics.md](docs/developers/perf/basics.md) is required reading for performance work.

## Repository map

| Path | Contents |
| --- | --- |
| `app/` | The `yak` binary and its crates (client, daemon, interpreter, analysis, execution, events). |
| `dice/` | DICE, the incremental computation engine. |
| `starlark-rust/` | The Starlark interpreter, parser, and LSP server. |
| `prelude/` | Starlark rules and toolchains, which the binary embeds. |
| `remote_execution/` | The Remote Execution API client. |
| `allocative/`, `gazebo/`, `pagable*/`, `shed/`, `superconsole/`, `host_sharing/` | Libraries the binary uses. |
| `YAK` files, `.yakconfig`, `build_defs/`, `toolchains/`, `third-party/` | The yak build of this repository. |
| `examples/` | Example projects to run a build against. |
| `tests/` | Integration tests (pytest) that run `target/debug/yak` against small projects. `tests/README.md` shows how to run them. |
| `website/` | The user documentation site. `website/docs/` holds its pages, and `website/gen_docs.py` generates the reference pages into it. |
| `docs/` | Contributor documentation (`docs/developers/`), the plan contract, execution plans, and the tech-debt tracker. The site does not publish it. |

## Build and verify

Run these commands from the repository root. `rust-toolchain.toml` pins the nightly toolchain, and `test.py` needs Python 3.

| Command | Result |
| --- | --- |
| `cargo build --bin=yak` | Builds `target/debug/yak`. |
| `python3 test.py <package>...` | Runs clippy and rustdoc with warnings denied, then unit and doc tests, for the named Cargo packages. Without packages it covers the whole workspace. |
| `cargo fmt --all` | Formats the workspace. CI does not check formatting, so run it before finishing. |
| `tests/.venv/bin/python -m pytest tests -n auto` | Runs the integration tests against `target/debug/yak`. `tests/README.md` sets up `tests/.venv`. |

CI (`.github/workflows/build-and-test.yml`) runs `cargo build --bin=yak` and then `python3 test.py --ci` on Linux, macOS, and Windows.
`.github/workflows/integration-tests.yml` runs the integration tests on Linux.
Run `test.py` for every package you changed. Run it without packages when a change reaches crates that many others depend on, such as `yak_core` or `yak_common`.
To check behavior end to end, run `target/debug/yak` in a project under `examples/` with its own `--isolation-dir`, as `docs/developers/basics.md` shows.

## Rules for changes

- The fork does not merge `facebook/buck2`. It ports single upstream commits under yak names, as `docs/developers/basics.md` describes.
- Code ported from `facebook/buck2` keeps only its open-source side. Drop `#[cfg(fbcode_build)]` branches, `@oss-disable` lines, `is_open_source()` checks, and `fbcode//` or `fbsource//` labels (`docs/developers/basics.md`).
- A dependency change updates both the crate's `Cargo.toml` and its `YAK` file. `docs/developers/basics.md` gives the steps, including `third-party/rust/` for new third-party crates.
- Check new crate dependencies against the late-binding and dependency rules in `ARCHITECTURE.md`. The yak build checks them (`docs/developers/basics.md`), but CI does not run the yak build.
- Read files, yakconfig, and other build state inside DICE computations only through DICE, so invalidation stays correct.
- `prelude/` changes reach a build only after the binary is rebuilt, because the binary embeds the prelude.
- When a golden test fails because the expected output changed, regenerate the golden file (`docs/developers/basics.md`) and review its diff.
- Update the documentation a change affects in the same change (user pages in `website/docs/`, contributor documentation in `docs/developers/`). Update `ARCHITECTURE.md` when a crate, boundary, or invariant it describes changes.

## Plans and debt

Work that spans sessions, crosses modules, or carries significant unknowns gets an execution plan. [docs/PLANS.md](docs/PLANS.md) defines the format and location.
Record problems found outside the current task in [the tech-debt tracker](docs/exec-plans/tech-debt-tracker.md).

## Finishing work

- An implementation is done when a test covers the changed behavior, `python3 test.py` passes for the affected packages, and the affected documentation is updated. Where no test harness can reach the behavior, record the command and output of a run against an example project.
- An investigation is done when a conclusion is supported by commands and output someone else can rerun, and the remaining uncertainty is stated.
- A review is done when each finding cites the file and line it is about, or the review states what it covered.
- Performance work needs a repeatable workload and measurements, as `docs/developers/perf/basics.md` describes.

## Skills

`.claude/skills/yak-rule-basics/` is an interactive tutorial for writing a first yak rule. Its `references/` directory summarizes the build model and common rule patterns.
