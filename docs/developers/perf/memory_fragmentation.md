---
id: perf_memory_fragmentation
title: Fragmentation Attribution
---

This page is about the `active - allocated` gap specifically — jemalloc
small-bin **slab fragmentation** — and how to attribute it back to the
allocation sites that cause it. For general allocator-stats and heap profiling
see [memory.md](memory.md).

## Decompose first

"Fragmentation" is three different problems; only one needs the tooling here.
Split the gap before doing anything else:

- **`resident - active`** — dirty/muzzy pages jemalloc hasn't returned to the
  OS. A decay-policy problem (`dirty_decay_ms`, `background_thread`, an explicit
  `arena.MALLCTL_ARENAS_ALL.purge`), not placement. Watch out for THP: with
  hugepages one live 4 KB region can pin 2 MB resident.
- **`active - allocated`** — slabs committed but not full: partially-empty slabs
  pinned by long-lived survivors. This is the placement problem typed arenas /
  lifetime segregation fix, and the subject of this page.
- **`allocated - requested`** — size-class rounding. jemalloc doesn't track
  requested bytes; a shim comparing `nallocx` to the requested size measures it.

On a representative daemon today `active - allocated` is ~2.7 GiB (≈22% of
active), essentially **all** of it small-bin slab waste.

## Step 1 — which size classes (`bin_waste.py`)

[`scripts/bin_waste.py`](scripts/bin_waste.py) parses jemalloc's per-bin
counters and ranks size classes by `active - allocated`:

```sh
buck2 debug allocator-stats -o J > stats.json   # -o J: do NOT suppress bin stats
scripts/bin_waste.py stats.json
# or: scripts/bin_waste.py --daemon
```

For each bin it prints utilization, total waste, and `live/nonfull` — the mean
number of live objects pinning each partially-empty slab. That number sorts the
bins into two regimes:

- **sparse** (few survivors per slab): a typed arena / lifetime pool for this
  type can free whole slabs. Arena candidate.
- **half-full** (slabs genuinely full of survivors): an arena won't help;
  segregate the *transients* sharing the bin instead.

This is a hypothesis, not proof — bin counters give the mean fill, not the
per-slab cohort structure. Step 2 confirms it and names the sites.

Note the **tcache caveat**: a region cached in a per-thread tcache still counts
as live (`curregs`), so the reported waste is a lower bound. For a precise
number start the daemon with `MALLOC_CONF=tcache:false`.

## Step 2 — which allocation sites

Knowing the bin isn't enough; you need the call sites whose survivors pin its
slabs. Finding them takes a sampling global allocator plus
`experimental.utilization.batch_query`, and this repository has no such
profiler. The method works as follows:

1. A `GlobalAlloc` wrapping the system allocator records a count-weighted sample
   of `(ptr, size, stack)` for live allocations and removes entries on free.
   jemalloc's `prof` does not work for this, because it promotes sampled small
   allocations to dedicated extents, which changes their placement.
2. At a fragmented steady state, it batch-queries `experimental.utilization` for
   every live sampled pointer. For each pointer jemalloc returns the slab's
   `nfree`, `nregs`, and extent `size`, and the region size is `size / nregs`.
3. It attributes each slab's waste to the survivors pinning it. Each sampled
   live pointer contributes `nfree·region_size / (r·live)` to its stack's total,
   an unbiased estimate of the per-stack pinned waste for sampling rate `r`.

A Rust `#[global_allocator]` sees only Rust allocations, so C++ survivors in a
slab stay unattributed.

## Known results

On a daemon after a `cquery` over buck2's own graph, about 1.2 GiB of small-bin
waste came almost entirely from `TargetNode` construction during loading and
attribute coercion. The attributed sites were coerced deps
(`ThinBoxSlice<TargetLabel>`, `Vec2<ProvidersLabel, …>`), `CoercedAttr::coerce`,
`AttrValues`, the `Arc<HeaderSlice<[CoercedAttr]>>` and target-label `Arc`s,
`TargetNode::new`, and `ArcStrInterner`. These long-lived graph nodes are allocated amid the
transient churn of evaluating each `BUCK` file, which makes them a candidate for
a per-package arena freed when the package's targets are invalidated.

In the 192-byte class, the class with the most waste, 32% of pages held
survivors from a single allocation site and 68% held a mix of that `TargetNode`
family (`ThinBoxSlice<TargetLabel>`, `AttrValues`, `CoercedAttr`, and
`Vec2<ProvidersLabel>`). `ThinBoxSlice<TargetLabel>` touched 26k pages but was
the only occupant of 6k, so an arena for any one of these types frees few slabs,
and the whole co-allocated family has to move together.
