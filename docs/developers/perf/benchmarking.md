---
id: perf_benchmarking
title: Benchmarking
---

[basics.md](basics.md) covers the perf basics shared with profiling. This
page is benchmarking-specific: variance, sample sizes, fair comparison
between two versions of yak or of the repo.

## Effect sizes

We typically aim to detect changes down to 0.3–0.5%; anything over 1% is a large win or regression.
Single-shot measurements detect ~nothing in this range — every benchmark needs many samples.

## Metrics and the daemon lifecycle

How the daemon is managed around each sample decides which metric answers which question:

| Question                                 | Daemon lifecycle                          | Metric                    |
|------------------------------------------|-------------------------------------------|---------------------------|
| "How long does yak take?"              | any (`--no-yakd` has the least variance) | Wall time                 |
| "How much memory at peak?"               | a fresh daemon for each sample            | Daemon `VmHWM` (peak RSS) |
| "Peak of a single `--no-yakd` process?" | `--no-yakd`                              | Max RSS of the process    |
| "How much does the daemon retain?"       | a fresh daemon for each sample            | jemalloc `allocated`      |
| "Does the daemon grow across commands?"  | one daemon reused across samples          | jemalloc `allocated`      |
| "Is fragmentation to blame?"             | fresh or reused                           | jemalloc `active - allocated` ([memory_fragmentation.md](memory_fragmentation.md)) |

- With a daemon, the max RSS of the invoked process describes the thin gRPC client
  ([basics.md](basics.md#the-process-model)), so read the daemon's numbers.
- With a reused daemon, `VmHWM` is the peak since the daemon started, not per sample.
- [`scripts/measure.sh`](scripts/measure.sh) takes one sample with a fresh daemon and records the
  daemon's `VmHWM`, `yak debug allocator-stats`, and a heap profile.

## Per-iteration variance

| Metric                     | Stddev across runs                |
|----------------------------|-----------------------------------|
| Wall time                  | 100 ms – 1 s on a 15 s build      |
| Daemon `VmHWM` (peak RSS)  | ~50 MB on a 4–5 GB build          |
| jemalloc `allocated`       | a few MB; very stable             |

`allocated` is stable enough that small samples are usable. For peak RSS and wall time, sub-1%
effects only emerge from pairing many samples across many hosts.

## Quick local checks

For coarse local iteration — effects of several percent, or checking that a workload behaves —
[absh](https://github.com/stepancheg/absh) is convenient:

```sh
cargo install --git https://github.com/stepancheg/absh absh
# -i: ignore the first iteration; -r: randomize A/B order; -m: max RSS of the spawned process
absh -a '/tmp/b2a ...' -b '/tmp/b2b ...' -i -r -m -n 30
```

`-m` is only meaningful with `--no-yakd`, where the spawned process is the one doing the work.
Run local loops in a benchmark-only worktree
([basics.md](basics.md#avoiding-daemon-conflicts)); switching binaries between iterations kills
the daemon via version skew, which conveniently gives fresh DICE each time.
