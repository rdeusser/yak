---
id: perf_basics
title: Performance Work Basics
---

This page is the unconditional read for anyone (human or agent) doing
performance work on Buck2. Memory profiling, benchmarking, regression
hunting, and CPU work all assume the model below.

The advice here is never to the exclusion of anything else, particularly not more advanced profiling
techniques. Treat it as a default and a starting point.

## The process model

A `yak` invocation is split between two processes:

- **Client**: the binary you actually invoke. Parses CLI args, talks gRPC to
  the daemon, prints the streamed events to your terminal, and exits when
  the command finishes.
- **Daemon (`yakd`)**: a long-lived process that holds the DICE graph,
  caches, executor connections, etc. All real work happens here.

Implications for measurement:

- `/usr/bin/time` on the client tells you almost nothing — the client is a
  thin gRPC shim. To measure CPU/RSS of the actual work, look at the
  daemon process.
- The daemon survives between invocations. Two consecutive `yak build` commands share DICE state,
  allocators, file watchers, etc. State carries across commands until the daemon is killed or
  restarted by version skew.
- `--no-buckd` runs everything in one process. When possible this is preferred as it's less noisy,
  but cannot be used for something like retained memory or with a pre-warming step.

## Avoiding daemon conflicts

Any one checkout can only have one daemon running at a time. If an existing daemon is running and
a new command is issued using a different version/build of yak, the existing daemon is killed.

 - Use `git worktree add` to create additional checkouts
 - If making changes to buck itself, don't run benchmarks in the same checkouts, it will make your
   rebuilds of buck slow.
 - Usually one checkout for all benchmarking is enough, unless you need persistent daemons
 - Isolation dirs offer some of the same behaviors but are not recommended because they split the
   remote action cache.

The first build in any checkout should normally be thrown out as it may need to populate kernel IO
caches.

## Metrics

Buck2 perf generally cares about three metrics: Peak memory, retained memory at the end of a
command, and command duration.

For memory, RSS is technically the metric that is important but jemalloc `allocated` or other stats
are acceptable proxies.

Profiling and benchmarking are different tasks; for details on benchmarking (variance, sample
sizes, comparing two binaries) see [benchmarking.md](benchmarking.md).

## Workloads

Choosing the right workload means higher SNR and faster results. *Typically:*

 - `yak cquery :target` to measure load (mostly), source file IO, and configuration.
 - `yak audit providers :target --quiet` to additionally measure analysis.
 - `yak build :target` to also measure execution.
   - This is the most complete picture.
   - Invoke with `-M none` unless specifically measuring materialization.
   - Ensure you get 100% cache hits. One build with `--remote-only` will generally populate anything
     missing and being on the right revision helps.

Additionally: `-v0`, `--console=none`, and `--no-buckd` helps reduce variance. Event log is still
produced.

This is not to the exclusion of anything else, other workloads may be appropriate depending on what
you're doing.

## Daemon management

Cargo builds of yak use jemalloc as the allocator on Linux and macOS and mimalloc on Windows. The
`release-symbols` profile in `Cargo.toml` adds debug symbols to a release build.

```sh
# Start a daemon if not yet started
yak server
# Stats about the daemon process, including pid at `.process_info.pid`
yak status
# Show jemalloc stats
yak debug allocator-stats
# Kill a daemon
yak kill
```

## Where to go next

- [memory.md](memory.md) — heap profiling, allocator stats, peak vs retained
- [memory_fragmentation.md](memory_fragmentation.md) — attributing the
  `active - allocated` slab-fragmentation gap to allocation sites (jemalloc only)
- [benchmarking.md](benchmarking.md) — making measurements meaningful
- [memory_regression_hunting.md](memory_regression_hunting.md) — comparing two binaries
- [scripts/](scripts/) — runnable tooling that wraps the recipes above
