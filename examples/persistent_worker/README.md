# Persistent Worker Demo

This demo builds the same actions locally and on BuildBuddy remote execution,
each with and without persistent workers. `test.sh` runs all four builds and
checks which executor ran each demo action.

## Requirements

This demo uses BuildBuddy remote execution to demonstrate remote persistent
workers. You will need an API token for at least a free open source account. You
can use [direnv] to set up the environment:

Credentials for [BuildBuddy] stored in `.envrc.private`:

```
export BUILDBUDDY_API_KEY=...
```

`test.sh` skips the remote execution builds when `BUILDBUDDY_API_KEY` is not
set. CI reads the key from the `BUILDBUDDY_API_KEY` secret.

[direnv]: https://direnv.net/
[BuildBuddy]: https://www.buildbuddy.io/

## Local Build

Configure a local build without persistent workers:

```
$ cd examples/persistent_worker
$ echo '<file:.yakconfig.no-workers>' > .yakconfig.local
```

Run a clean build:

```
$ yak clean; yak build : -vstderr
...
stderr for root//:demo-3 (demo):
...
ONE-SHOT START
...
```

## Local Persistent Worker

Configure a local build with persistent workers:

```
$ cd examples/persistent_worker
$ echo '<file:.yakconfig.local-persistent-workers>' > .yakconfig.local
```

Run a clean build:

```
$ yak clean; yak build : -vstderr
...
stderr for root//:demo-3 (demo):
...
yak persistent worker ...
...
```

## Remote Execution

Configure a remote build without persistent workers:

```
$ cd examples/persistent_worker
$ echo '<file:.yakconfig.buildbuddy>' > .yakconfig.local
```

Run a clean build:

```
$ yak clean; yak build : -vstderr
...
stderr for root//:demo-3 (demo):
...
ONE-SHOT START
...
```

## Remote Persistent Worker

Configure a remote build with persistent workers:

```
$ cd examples/persistent_worker
$ echo '<file:.yakconfig.buildbuddy-persistent-workers>' > .yakconfig.local
```

Run a clean build:

```
$ yak clean; yak build : -vstderr
...
stderr for root//:demo-3 (demo):
...
Bazel persistent worker ...
...
```

## Protocol

### Starlark

A yak persistent worker is created by a rule that emits the `WorkerInfo`
provider. Setting `supports_bazel_remote_persistent_worker_protocol = True` on
this provider indicates that this worker is remote execution capable.

yak actions indicate that they can utilize a persistent worker by setting the
`exe` parameter to `ctx.actions.run` to `WorkerRunInfo(worker, exe)`, where
`worker` is a `WorkerInfo` provider, and `exe` defines the fallback executable
for non persistent-worker execution.

yak actions that want to utilize a remote persistent worker must pass
command-line arguments in an argument file specified as `@argfile`,
`-flagfile=argfile`, or `--flagfile=argfile` on the command-line.

### Local Persistent Worker

A locally executed yak persistent worker falls under the
[yak persistent worker protocol](./proto/yak_worker/worker.proto): It is started
and managed by yak and passed a file path in the `WORKER_SOCKET` environment
variable where it should create a gRPC Unix domain socket to serve worker
requests over. Multiple requests may be sent in parallel and expected to be
served at the same time depending on the `concurrency` attribute of the
`WorkerInfo` provider.

### Remote Persistent Worker

A remotely executed yak persistent worker falls under the
[Bazel persistent worker protocol](./proto/bazel/worker_protocol.proto): It is
started and managed by the remote execution system. Work requests are sent as
length prefixed protobuf objects to the standard input of the worker process.
Work responses are expected as length prefixed protobuf objects on the standard
output of the worker process. The worker process may not use standard output for
anything else.
