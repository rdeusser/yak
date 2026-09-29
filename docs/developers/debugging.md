# Debugging

This page describes how contributors usually debug Buck2. It is descriptive, so use another approach where it works better.

## Logic bugs

Most logic bugs are found by reading the code, writing finer-grained tests, or adding `println!` calls. A traditional debugger works with the standard tools but is rarely used.

`yak log`, `yak audit`, and `yak debug` report what a build did.

## Running local changes

[basics.md](./basics.md) shows how to build `target/debug/yak` and run it in a test project with its own isolation directory.

`./yak.py <command>` builds `//:yak_bundle` with the `yak` on `PATH` and runs `<command>` with the result. It needs the Buck build of this repository, which in turn needs a `yak` on `PATH` built from this repository ([basics.md](./basics.md)). The command runs in the isolation directory `v2.self`, which keeps it away from your existing daemon, but large builds get no cache hits there and run slowly.

## Event logs

Every command writes an event log to `yak-out/<isolation dir>/log/`. The `yak log` subcommands read the last invocation's log unless told otherwise:

```bash
# Commands the last invocation ran, and how to rerun each one
yak log what-ran
# Commands that failed
yak log what-failed
# The slowest chain of actions
yak log critical-path
# The whole log as JSON
yak log show
# The path of the log file
yak log path
```

[what-ran.md](./what-ran.md) explains the `what-ran` output.

## Daemon state

The daemon keeps `yakd.info` (endpoint and pid), `yakd.pid`, `yakd.stdout`, and `yakd.stderr` in `~/.yak/yakd/<project root>/<isolation dir>/`. The daemon writes panics and its tracing output to `yakd.stderr`. A daemon that exits with an error writes a JSON error report to `yakd.error.log`, which the client reads to explain a failed start.

```bash
# Daemon pid and state
yak status
# Stop the daemon
yak kill
```

## Tracing

Buck2 also emits sparse `tracing` output. `YAK_LOG` sets the filter, using the [`EnvFilter` syntax](https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/filter/struct.EnvFilter.html). The daemon reads `YAK_LOG` only when it starts, so restart it to change the filter:

```bash
yak kill
YAK_LOG=module_name=trace yak <command>
# Examples
YAK_LOG=starlark=trace yak uquery cell//path/to:target
YAK_LOG=buck2_execute_impl::materializers=trace yak build cell//path/to:target
```

The client prints its own tracing output to the terminal, and the daemon writes its output to `yakd.stderr`. With `--no-yakd`, the daemon runs inside the client process and its output also goes to the terminal.

## Tests

`yak test` runs tests through the built-in test executor (`yak internal-test-runner`) unless `[test] v2_test_executor` names another executable. `buck2_test` logs each line the executor prints to stdout or stderr at `debug` level, so `YAK_LOG=buck2_test=debug` shows them while you print-debug a test executor.
