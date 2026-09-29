# Integration tests

These pytest tests run a `yak` binary against small projects and check its output, its event logs, and the files it writes.

## Layout

| Path | Contents |
| --- | --- |
| `core/` | Tests of yak itself, grouped by the behavior they check. `core/README.md` has guidelines for writing them. |
| `e2e/` | Tests that combine yak with the prelude and other integrations. |
| `prelude/` | Tests of prelude rules. |
| `select_type_params/`, `tools/` | Tests of smaller pieces. |
| `e2e_util/` | The harness. It holds the `buck` fixture, the `Buck` command wrapper, assertions, golden file helpers, and `nano_prelude`. |

`nano_prelude` is a small prelude for projects that do not need the real one.

## Running the tests

The tests need Python 3.12 or later and a debug build of yak. Run these commands from the repository root:

```sh
cargo build --bin=yak
python3 -m venv tests/.venv
tests/.venv/bin/pip install -r tests/requirements.txt
tests/.venv/bin/python -m pytest tests -n auto
```

Pass a file, a directory, or a test id to run fewer tests:

```sh
tests/.venv/bin/python -m pytest tests/core/build/test_out_flag.py
tests/.venv/bin/python -m pytest "tests/core/build/test_out_flag.py::test_out_single_default_output"
```

The tests run `target/debug/yak`. Set `YAK_BINARY` to an absolute path to test another binary.

These tests fail when a program or condition they rely on is missing:

- `test_process_title` in `core/daemon/test_daemon.py` and `core/daemon/test_daemon_killed_on_checkout_removal.py` run `ps`.
- `test_thread_dump` in `core/debug/test_debug.py` runs `lldb`, which attaches to the daemon. On Linux with Yama, a user other than root needs `kernel.yama.ptrace_scope` set to 0 for the attach, because the daemon is not a child of `lldb`.
- `test_clean_scratch_on_idle` in `core/materializer/test_clean_stale.py` needs a user other than root, because root can read a directory whose permissions deny reading.
- On Linux, `test_current_oomd_record_marks_daemon_crash_as_oom` in `core/build/test_error_categorization.py` needs the daemon in a cgroup below the root of its cgroup namespace.
- `test_go` in `prelude/test_prelude_rules.py` fails with a Go older than 1.23, because the prelude's Go tools use newer standard library functions. On Linux, its cgo and external linking packages also need `clang` and `lld`, because the system C++ toolchain links with `-fuse-ld=lld` (`prelude/toolchains/cxx.bzl`).

Each test copies its project into a new temporary directory and starts its own daemon, which keeps its state under that directory. The test kills the daemon and deletes the directory when it finishes. Set `YAK_E2E_KEEP_TEMP=1` to keep the directory, and the test prints its path.

## Skipped tests

Some tests need resources that a developer machine usually lacks. They carry a marker, and pytest reports them as skipped with the reason.

| Marker | Needs | Runs when |
| --- | --- | --- |
| `remote_execution` | A Remote Execution backend. | `YAK_TEST_RE_CONFIG` names a buckconfig file with the backend's `[yak_re_client]` settings. The harness appends the file to every test project's `.yakconfig`. |
| `cgroups` | Linux with a systemd user session that delegates cgroups, because the daemon moves itself into a cgroup with `systemd-run --user`. `@buck_test(disable_daemon_cgroup=False)` adds this marker. | `YAK_TEST_CGROUPS=1` is set. |
| `needs_binary` | Helper programs named by environment variables. | Every variable that the marker names is set. |

This repository has no Remote Execution backend to test against, so the `remote_execution` tests are unverified. Some of them build a project that declares no execution platforms, and yak does not run the actions of such a project remotely even when a backend is configured. Those projects need a remote-enabled execution platform before their tests can pass.

The helper programs:

| Variable | Program | How to build it |
| --- | --- | --- |
| `THREE_BILLION_INSTRUCTIONS_BIN` | `shed/three_billion_instructions` | `cargo build -p three_billion_instructions --bin three_billion_instructions_bin` |
| `SKETCH_SIZE_BIN` | `shed/setsketch` | `cargo build -p setsketch --bin sketch_size` |
| `USE_SOME_MEMORY_BIN` | `shed/cgroups/use_some_memory` | It has a Buck target but no Cargo target. |
| `YAK_COMPLETION_VERIFY` | `shed/completion_verify` | It has a Buck target but no Cargo target. |
| `INSTALLER_BIN`, `FORWARDED_PARAMS_INSTALLER_BIN`, `EXTRA_ARGS_VALIDATOR_BIN`, `EARLY_EXIT_INSTALLER_BIN` | Installers for the `yak install` tests. | Their sources are not in this repository. |

The Watchman tests in `core/io/` skip when `watchman` is not on `PATH`. The Go tests in `prelude/` skip when `go` is not on `PATH`.

Tests can also skip an operating system with `@buck_test(skip_for_os=[...])`.

## Golden files

A golden test compares output with a checked-in file after it replaces timestamps, digests, and temporary paths with placeholders. When a change alters that output on purpose, rerun the failing tests with `YAK_UPDATE_GOLDEN=1` to rewrite their golden files:

```sh
YAK_UPDATE_GOLDEN=1 tests/.venv/bin/python -m pytest tests/core/help -n auto
```

The update accepts whatever the binary printed, so review the diff of the golden files before you commit them.

## Writing a test

Copy an existing test and its data directory, and change them. A test module `test_foo.py` reads its projects from `test_foo_data/` next to it:

```python
from e2e_util.api.buck import Buck
from e2e_util.buck_workspace import buck_test


@buck_test()
async def test_build_writes_output(buck: Buck) -> None:
    result = await buck.build("root//:target")
    output = result.get_build_report().output_for_target("root//:target")
    assert output.read_text() == "hello\n"
```

`@buck_test()` copies the data directory into the temporary project, and `data_dir="name"` copies only its subdirectory `name`. Its other arguments add buckconfig values, allow soft errors, and skip operating systems. The docstring of `buck_test` in `e2e_util/buck_workspace.py` lists them.

The data directories name their build files `YAK.fixture` through `[buildfile] name` in the project's `.yakconfig`. That name keeps this repository's own Buck build from reading them. `PACKAGE` files keep their usual name.

A project uses `nano_prelude` with this `.yakconfig`:

```ini
[cells]
  root = .
  nano_prelude = nano_prelude

[cell_aliases]
  prelude = nano_prelude

[external_cells]
  nano_prelude = bundled

[buildfile]
  name = YAK.fixture
```

A project that needs the real prelude uses `prelude = bundled` under `[external_cells]` instead. The binary embeds the prelude from `prelude/`, so a prelude change reaches these tests only after `cargo build --bin=yak`.
