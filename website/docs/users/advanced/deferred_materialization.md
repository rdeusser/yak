---
id: deferred_materialization
title: Deferred Materialization
---

When using [Remote Execution](../remote_execution.md), yak operates with
Deferred Materialization, which means that yak will avoid downloading outputs
until they are required by a local action.

This can provide very substantial performance savings on builds that execute
primarily on Remote Execution, since those builds become able to proceed without
ever downloading any intermediary outputs.

## Pitfalls

yak's deferred materialization makes assumptions about your Remote Execution
backend. In particular, it expects that the TTL returned from action cache
entries by your Remote Execution backend always exceeds the TTL of all output
artifacts it references.

Nonetheless, artifacts may also eventually expire from your Remote Execution
backend. When that happens, builds using Deferred Materialization may fail if
those artifacts are needed locally.

A kill is necessary to recover from those builds. However, the
[Restarter](restarter.md) can be used to mitigate this issue by restarting yak
daemon when it encounters an expired artifact.

The deferred materializer can refresh artifact TTLs periodically, but the
refresh does not work with open-source Remote Execution backends, which do not
expose the TTL of artifacts.

## On-disk state

yak tracks the files it has written to `yak-out` in a SQLite database, so it
remembers them across daemon restarts and does not download outputs from a
Remote Execution backend again when they are already on disk.

yak also records each command that ran locally in a second database, with the
digests of its command line and inputs and the outputs it produced. After a
restart, a command whose command line and inputs match a recorded one reuses
its outputs, as long as they are still in `yak-out`, and does not run again. A
command that declares dep files reuses them only when all of its inputs match,
because the comparison of the inputs its dep files name does not survive a
restart. An entry is pruned 7 days after its command last ran, which
`sqlite_dep_file_state_ttl_days` changes, and `sqlite_dep_file_state_max_entries`
limits the number of entries.

Both databases are on by default. The record of actions needs the materializer
state. To turn them off, add this to your yakconfig:

```ini
[yak]
sqlite_materializer_state = false
sqlite_dep_file_state = false
```

## Deferring Write Actions

To further speedup builds, yak can also be instructed to not execute any
writes on the critical path for a build.

To enable, add this to your yakconfig:

```ini
[yak]
defer_write_actions = true
```

This mechanism is recommended if you're using the On-disk State, since it means
yak can omit writes entirely if the same content is already on disk.

## `yak clean --stale`

The deferred materializer can be configured to continuously delete stale
artifacts, that haven't been recently accessed, or untracked artifacts, that
exist in yak-out but not in the materalizer state.

Unlike `yak clean` this does not fully wipe yak-out but it should not
negatively impact build performance if you are building and rebasing regularly.

Enabling this requires enabling [on-disk state](#on-disk-state) and
[deferred write actions](#deferring-write-actions), and adding this to your
yakconfig:

```ini
[yak]
clean_stale_enabled = true
```

It can be further configured by changing these default values:

```ini
[yak]
# one week
clean_stale_artifact_ttl_hours = 24 * 7
clean_stale_period_hours = 24
clean_stale_start_offset_hours = 12
```

- `clean_stale_start_offset_hours` determines the time following daemon start up
  before the first clean will be scheduled.
- `clean_stale_period_hours` determines how frequently to schedule recurring
  clean events.
- `clean_stale_artifact_ttl_hours` determines how long artifacts should be kept
  in yak-out before cleaning them.
- `clean_stale_dry_run` (default false) reports what would be cleaned without
  deleting it.
- `clean_stale_low_disk_threshold` (percent of total disk free, e.g. `10.0`)
  enables more aggressive cleaning when free disk drops at or below it. The
  low-disk behavior below never engages unless this is set.
- `clean_stale_low_disk_artifact_ttl_hours` (default 48) sets a shorter fixed
  TTL to use while free disk is at or below the threshold.
- `clean_stale_low_disk_adaptive_enabled` (default false) replaces the fixed
  shorter TTL with adaptive cleaning: yak keeps promoting the oldest retained
  artifacts to stale until projected free disk rises back above the threshold,
  while protecting any artifact younger than
  `clean_stale_low_disk_adaptive_min_ttl_hours` (default 12).
- `clean_stale_low_disk_adaptive_delete_intermediate_within_min_ttl` (default
  false) allows adaptive cleaning to delete non-active artifacts marked as
  intermediate-only even when they are below the adaptive minimum TTL.
- `clean_stale_low_disk_adaptive_unmaterialize_active` (default false) adds a
  final adaptive escalation step that discards active intermediate artifacts
  backed by CAS or HTTP downloads. Their materializer entries return to the
  declared state, so a later build downloads them again without rerunning the
  producing action.
- `clean_stale_low_disk_unmaterialization_threshold` (defaults to
  `clean_stale_low_disk_threshold`) sets the free-disk percentage that active
  unmaterialization recovers to. It must not exceed
  `clean_stale_low_disk_threshold` and only applies when
  `clean_stale_low_disk_adaptive_unmaterialize_active` is enabled. Active
  unmaterialization is also suppressed unless `ttl_refresh_enabled` is enabled
  for the daemon, because CAS blobs must remain available for future
  rematerialization.

If clean stale is running in the background at the same time that a build begins
to materialize artifacts, the clean will be interrupted and not run again until
after the next scheduled period, but it should be able to make gradual progress
and prevent long term accumulation of artifacts.

If needed, a clean can be manually triggered by calling `yak clean --stale`.
With no cleanup-policy flags, the command uses the configured artifact TTL,
dry-run setting, and fixed or adaptive low-disk policy. These settings apply to
manual cleanup even when `clean_stale_enabled` is false; that setting only
controls periodic scheduling.

An explicit duration or adaptive low-disk flag selects an explicit command-line
policy instead. The equivalent explicit manual escalation is
`--adaptive-unmaterialize-active`, which requires
`--adaptive-low-disk-threshold` and the same TTL-refresh support.
