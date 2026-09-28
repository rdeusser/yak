# Debugging

This page describes how contributors usually debug Buck2. It is descriptive, so use another approach where it works better.

## Logic bugs

Most logic bugs are found by reading the code, writing finer-grained tests, or adding `println!` calls. A traditional debugger works with the standard tools but is rarely used.

`buck2 log`, `buck2 audit`, and `buck2 debug` report what a build did.

## Running local changes

[basics.md](./basics.md) shows how to build `target/debug/buck2` and run it in a test project with its own isolation directory.

`./buck2.py <command>` builds `//:buck2_bundle` with the `buck2` on `PATH` and runs `<command>` with the result. It needs the Buck build of this repository, which in turn needs a `buck2` on `PATH` built from this repository ([basics.md](./basics.md)). The command runs in the isolation directory `v2.self`, which keeps it away from your existing daemon, but large builds get no cache hits there and run slowly.

## Event logs

Every command writes an event log to `buck-out/<isolation dir>/log/`. The `buck2 log` subcommands read the last invocation's log unless told otherwise:

```bash
# Commands the last invocation ran, and how to rerun each one
buck2 log what-ran
# Commands that failed
buck2 log what-failed
# The slowest chain of actions
buck2 log critical-path
# The whole log as JSON
buck2 log show
# The path of the log file
buck2 log path
```

[what-ran.md](./what-ran.md) explains the `what-ran` output.

## Daemon state

The daemon keeps `buckd.info` (endpoint and pid), `buckd.pid`, `buckd.stdout`, and `buckd.stderr` in `~/.buck/buckd/<project root>/<isolation dir>/`. The daemon writes panics and its tracing output to `buckd.stderr`. A daemon that exits with an error writes a JSON error report to `buckd.error.log`, which the client reads to explain a failed start.

```bash
# Daemon pid and state
buck2 status
# Stop the daemon
buck2 kill
```

## Tracing

Buck2 also emits sparse `tracing` output. `BUCK_LOG` sets the filter, using the [`EnvFilter` syntax](https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/filter/struct.EnvFilter.html). The daemon reads `BUCK_LOG` only when it starts, so restart it to change the filter:

```bash
buck2 kill
BUCK_LOG=module_name=trace buck2 <command>
# Examples
BUCK_LOG=starlark=trace buck2 uquery cell//path/to:target
BUCK_LOG=buck2_execute_impl::materializers=trace buck2 build cell//path/to:target
```

The client prints its own tracing output to the terminal, and the daemon writes its output to `buckd.stderr`. With `--no-buckd`, the daemon runs inside the client process and its output also goes to the terminal.

## Tests

`buck2 test` runs tests through the built-in test executor (`buck2 internal-test-runner`) unless `[test] v2_test_executor` names another executable. `buck2_test` logs each line the executor prints to stdout or stderr at `debug` level, so `BUCK_LOG=buck2_test=debug` shows them while you print-debug a test executor.
