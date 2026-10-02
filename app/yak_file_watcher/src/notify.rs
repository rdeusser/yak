/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::mem;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceTransactionUpdater;
use dupe::Dupe;
use notify::EventKind;
use notify::RecommendedWatcher;
#[cfg(not(target_os = "macos"))]
use notify::Watcher;
use notify::event::CreateKind;
use notify::event::MetadataKind;
use notify::event::ModifyKind;
use notify::event::RemoveKind;
use starlark_map::ordered_set::OrderedSet;
use tokio::task::spawn_blocking;
use tracing::debug;
use tracing::info;
use tracing::warn;
use yak_common::file_ops::dice::FileChangeTracker;
use yak_common::ignores::ignore_set::IgnoreSet;
use yak_common::invocation_paths::InvocationPaths;
use yak_core::cells::CellResolver;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::name::CellName;
use yak_core::fs::project::ProjectRoot;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_data::FileWatcherEventType;
use yak_data::FileWatcherKind;
use yak_error::conversion::from_any_with_tag;
use yak_events::dispatch::span_async;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_hash::StdYakHashMap;

use crate::file_watcher::FileWatcher;
use crate::mergebase::Mergebase;
use crate::rescan::Snapshot;
use crate::stats::FileWatcherStats;

fn ignore_event_kind(event_kind: EventKind) -> bool {
    match event_kind {
        EventKind::Access(_) => true,
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::Ownership))
        | EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)) => false,
        EventKind::Modify(ModifyKind::Metadata(_)) => true,
        _ => false,
    }
}

/// Buffer containing the events that have happened since we last got a message.
/// Used to dedupe events, since notify sends a notification on every change.
#[derive(Allocative)]
struct NotifyFileData {
    ignored: u64,
    #[allocative(skip)]
    events: OrderedSet<(CellPath, EventKind)>,
    /// The paths of `events`, which the snapshot brings up to date.
    #[allocative(skip)]
    paths: OrderedSet<ProjectRelativePathBuf>,
    /// Whether file system changes were missed
    missed_events: bool,
}

impl NotifyFileData {
    fn new() -> Self {
        Self {
            ignored: 0,
            events: OrderedSet::new(),
            paths: OrderedSet::new(),
            missed_events: false,
        }
    }

    fn process(
        &mut self,
        event: notify::Result<notify::Event>,
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
    ) -> yak_error::Result<()> {
        let event = event.map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::NotifyWatcher))?;

        // Checked before the path loop: the kernel-overflow rescan signal can arrive with no
        // paths attached (platform dependent), and a missed rescan means the daemon keeps
        // answering from a stale graph.
        if event.need_rescan() {
            self.missed_events = true;
            debug!("FileWatcher: File change events were missed");
        }

        for path in &event.paths {
            // Testing shows that we get absolute paths back from the `notify` library.
            // It's not documented though.
            let path = root.relativize(AbsNormPath::new(&path)?)?;

            // We ignore the yak-out prefix, as those are uninteresting events caused by us.
            // We also ignore other yak-out directories, as if you have two isolation dirs running at once, they are not interesting.
            // We do this in the notify-watcher, rather than a generic layer, as watchman users should configure
            // to ignore yak-out, to reduce the number of events, rather than hiding them later.
            if path.starts_with(InvocationPaths::yak_out_dir_prefix()) {
                // We don't want to event add them as ignored events, since they are super common
                // and very boring
                continue;
            }

            let cell_path = cells.get_cell_path(&path);
            let ignore = ignore_specs
                .get(&cell_path.cell())
                // See the comment on the analogous code in `watchman/interface.rs`
                .is_some_and(|ignore| ignore.is_match(cell_path.path()));

            info!(
                "FileWatcher: {:?} {:?} (ignore = {})",
                path, &event.kind, ignore
            );

            if ignore || ignore_event_kind(event.kind) {
                self.ignored += 1;
            } else {
                self.events.insert((cell_path, event.kind));
                self.paths.insert(path.into_owned());
            }
        }
        Ok(())
    }

    fn sync(self) -> Synced {
        // The changes that go into the DICE transaction
        let mut changed = FileChangeTracker::new();
        let mut stats = FileWatcherStats::new(Default::default(), self.events.len());
        stats.add_ignored(self.ignored);

        for (cell_path, event_kind) in self.events {
            let cell_path_str = cell_path.to_string();
            match event_kind {
                EventKind::Create(create_kind) => match create_kind {
                    CreateKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                    }
                    CreateKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                    CreateKind::Any | CreateKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Modify(modify_kind) => match modify_kind {
                    ModifyKind::Data(_) | ModifyKind::Metadata(_) => {
                        changed.file_contents_changed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Modify,
                            FileWatcherKind::File,
                        );
                    }
                    ModifyKind::Name(_) | ModifyKind::Any | ModifyKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Remove(remove_kind) => match remove_kind {
                    RemoveKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                    }
                    RemoveKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                    RemoveKind::Any | RemoveKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                _ => {}
            }
        }

        Synced {
            stats,
            changed,
            paths: self.paths.into_iter().collect(),
            missed_events: self.missed_events,
        }
    }
}

/// The stats of a sync that drops the DICE graph because it cannot tell which files changed,
/// reported with the fields that watchman's fresh instance uses for the same wipe.
fn clear_graph(mut stats: FileWatcherStats, reason: String) -> yak_data::FileWatcherStats {
    let base = stats.base_mut();
    base.fresh_instance = true;
    base.fresh_instance_data = Some(yak_data::FreshInstance {
        new_mergebase: false,
        cleared_dice: true,
        cleared_dep_files: false,
    });
    base.incomplete_events_reason = Some(reason);
    stats.finish()
}

/// The events of one sync, and whether the operating system dropped some.
struct Synced {
    stats: FileWatcherStats,
    changed: FileChangeTracker,
    paths: Vec<ProjectRelativePathBuf>,
    missed_events: bool,
}

/// The snapshot of the project that the watcher keeps current from its events.
enum SnapshotState {
    /// No sync has run, so DICE holds no state of the project's files.
    Initial,
    Current(Snapshot),
    /// A crawl or an update of the snapshot failed, so the snapshot cannot name the paths under a
    /// changed directory.
    Lost,
}

/// The source of file system events, which delivers them until it is dropped.
enum EventSource {
    #[cfg(target_os = "macos")]
    FsEvents(#[expect(dead_code)] crate::fsevents::FsEventsWatcher),
    #[cfg_attr(target_os = "macos", expect(dead_code))]
    Notify(#[expect(dead_code)] RecommendedWatcher),
}

#[derive(Allocative)]
pub struct NotifyFileWatcher {
    #[allocative(skip)]
    #[expect(dead_code)]
    events: EventSource,
    data: Arc<Mutex<yak_error::Result<NotifyFileData>>>,
    root: ProjectRoot,
    #[allocative(skip)]
    cells: CellResolver,
    #[allocative(skip)]
    ignore_specs: Arc<StdYakHashMap<CellName, IgnoreSet>>,
    /// The project's files as of the last sync. The first sync crawls the project, and each later
    /// sync applies its events.
    #[allocative(skip)]
    snapshot: Mutex<SnapshotState>,
}

impl NotifyFileWatcher {
    pub fn new(
        root: &ProjectRoot,
        cells: CellResolver,
        ignore_specs: StdYakHashMap<CellName, IgnoreSet>,
    ) -> yak_error::Result<Self> {
        let data = Arc::new(Mutex::new(Ok(NotifyFileData::new())));
        let data2 = data.dupe();
        let root2 = root.dupe();
        let cells2 = cells.dupe();
        let ignore_specs = Arc::new(ignore_specs);
        let ignore_specs2 = ignore_specs.dupe();
        let handler = move |event| {
            let mut guard = data2.lock().unwrap();
            if let Ok(state) = &mut *guard {
                if let Err(e) = state.process(event, &root2, &cells2, &ignore_specs2) {
                    *guard = Err(e);
                }
            }
        };
        let events = Self::watch(root, handler)?;
        Ok(Self {
            events,
            data,
            root: root.dupe(),
            cells,
            ignore_specs,
            snapshot: Mutex::new(SnapshotState::Initial),
        })
    }

    /// On macOS, the FSEvents stream leaves out `yak-out`, whose events can overflow the stream
    /// during a build and make FSEvents drop events of the project.
    #[cfg(target_os = "macos")]
    fn watch(
        root: &ProjectRoot,
        handler: impl FnMut(notify::Result<notify::Event>) + Send + 'static,
    ) -> yak_error::Result<EventSource> {
        let root = root.root().as_path();
        let yak_out = root.join(InvocationPaths::yak_out_dir_prefix().as_str());
        Ok(EventSource::FsEvents(
            crate::fsevents::FsEventsWatcher::new(root, &[yak_out], Box::new(handler))?,
        ))
    }

    #[cfg(not(target_os = "macos"))]
    fn watch(
        root: &ProjectRoot,
        handler: impl FnMut(notify::Result<notify::Event>) + Send + 'static,
    ) -> yak_error::Result<EventSource> {
        let mut watcher = notify::recommended_watcher(handler)
            .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::NotifyWatcher))?;
        watcher
            .watch(root.root().as_path(), notify::RecursiveMode::Recursive)
            .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::NotifyWatcher))?;
        Ok(EventSource::Notify(watcher))
    }

    async fn crawl(&self) -> yak_error::Result<Snapshot> {
        let root = self.root.dupe();
        let cells = self.cells.dupe();
        let ignore_specs = self.ignore_specs.dupe();
        spawn_blocking(move || Snapshot::crawl(&root, &cells, &ignore_specs)).await?
    }

    /// Applies the paths of a sync's events to the snapshot, and records the changes under the
    /// directories they name.
    async fn apply(
        &self,
        mut snapshot: Snapshot,
        paths: Vec<ProjectRelativePathBuf>,
        mut changed: FileChangeTracker,
        mut stats: FileWatcherStats,
    ) -> yak_error::Result<(Snapshot, FileChangeTracker, FileWatcherStats)> {
        let root = self.root.dupe();
        let cells = self.cells.dupe();
        let ignore_specs = self.ignore_specs.dupe();
        spawn_blocking(move || {
            snapshot.apply(
                paths,
                &root,
                &cells,
                &ignore_specs,
                &mut changed,
                &mut stats,
            )?;
            Ok((snapshot, changed, stats))
        })
        .await?
    }

    async fn sync2(
        &self,
        mut dice: DiceTransactionUpdater,
    ) -> yak_error::Result<(yak_data::FileWatcherStats, DiceTransactionUpdater)> {
        let old = {
            let mut guard = self.data.lock().unwrap();
            mem::replace(&mut *guard, Ok(NotifyFileData::new()))
        };
        let Synced {
            mut stats,
            mut changed,
            paths,
            missed_events,
        } = old?.sync();

        // A failure below leaves the state lost.
        let previous = mem::replace(&mut *self.snapshot.lock().unwrap(), SnapshotState::Lost);
        let (next, clear_reason) = match (previous, missed_events) {
            (SnapshotState::Current(snapshot), false) => {
                match self.apply(snapshot, paths, changed, stats).await {
                    Ok((snapshot, applied, applied_stats)) => {
                        changed = applied;
                        stats = applied_stats;
                        (SnapshotState::Current(snapshot), None)
                    }
                    Err(e) => {
                        warn!("FileWatcher: Could not update the snapshot of the project: {e:#}");
                        // The tracker and stats moved into the failed update.
                        changed = FileChangeTracker::new();
                        stats = FileWatcherStats::new(Default::default(), 0);
                        (
                            SnapshotState::Lost,
                            Some(format!(
                                "notify could not update its snapshot of the project: {e:#}"
                            )),
                        )
                    }
                }
            }
            // The operating system dropped events, so the files that changed since the last
            // sync are found by crawling again.
            (SnapshotState::Current(previous), true) => match self.crawl().await {
                Ok(snapshot) => {
                    let count = previous.changes(&snapshot, &self.cells, &mut changed, &mut stats);
                    stats.base_mut().incomplete_events_reason = Some(format!(
                        "notify dropped events, and a rescan of the project found {count} changed paths"
                    ));
                    (SnapshotState::Current(snapshot), None)
                }
                Err(e) => {
                    warn!("FileWatcher: Could not crawl the project: {e:#}");
                    (
                        SnapshotState::Lost,
                        Some(format!(
                            "notify dropped events, and a rescan of the project failed: {e:#}"
                        )),
                    )
                }
            },
            // The first sync takes the snapshot that later syncs keep current.
            (SnapshotState::Initial, missed_events) => {
                let next = match self.crawl().await {
                    Ok(snapshot) => SnapshotState::Current(snapshot),
                    Err(e) => {
                        warn!("FileWatcher: Could not crawl the project: {e:#}");
                        SnapshotState::Lost
                    }
                };
                let reason =
                    missed_events.then(|| "notify dropped events before its first sync".to_owned());
                (next, reason)
            }
            // Without a snapshot, the changes under a directory are unknown.
            (SnapshotState::Lost, _) => {
                let next = match self.crawl().await {
                    Ok(snapshot) => SnapshotState::Current(snapshot),
                    Err(e) => {
                        warn!("FileWatcher: Could not crawl the project: {e:#}");
                        SnapshotState::Lost
                    }
                };
                (
                    next,
                    Some("notify has no snapshot of the project from the last sync".to_owned()),
                )
            }
        };
        *self.snapshot.lock().unwrap() = next;
        if let Some(reason) = clear_reason {
            return Ok((clear_graph(stats, reason), dice.unstable_take()));
        }
        changed.write_to_dice(&mut dice)?;
        Ok((stats.finish(), dice))
    }
}

#[async_trait]
impl FileWatcher for NotifyFileWatcher {
    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> yak_error::Result<(DiceTransactionUpdater, Mergebase)> {
        span_async(
            yak_data::FileWatcherStart {
                provider: yak_data::FileWatcherProvider::RustNotify as i32,
            },
            async {
                let (stats, res) = match self.sync2(dice).await {
                    Ok((stats, dice)) => {
                        let mergebase = Mergebase(Arc::new(stats.branched_from_revision.clone()));
                        ((Some(stats)), Ok((dice, mergebase)))
                    }
                    Err(e) => (None, Err(e)),
                };
                (res, yak_data::FileWatcherEnd { stats })
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use notify::event::CreateKind;
    use notify::event::Flag;
    use yak_core::cells::cell_root_path::CellRootPathBuf;
    use yak_core::cells::name::CellName;
    use yak_core::fs::project_rel_path::ProjectRelativePath;
    use yak_fs::fs_util::uncategorized as fs_util;
    use yak_fs::paths::abs_norm_path::AbsNormPathBuf;

    use super::*;

    fn fixture() -> (
        ProjectRoot,
        CellResolver,
        StdYakHashMap<CellName, IgnoreSet>,
        tempfile::TempDir,
    ) {
        let cells = CellResolver::testing_with_name_and_path(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        );
        let tempdir = tempfile::tempdir().unwrap();
        let root_path =
            fs_util::canonicalize(AbsNormPathBuf::new(tempdir.path().to_owned()).unwrap()).unwrap();
        let root = ProjectRoot::new(root_path).unwrap();
        (root, cells, StdYakHashMap::default(), tempdir)
    }

    /// The kernel-overflow signal can arrive as a rescan-flagged event with no paths attached
    /// (platform dependent). The flag must still be recorded, otherwise lost notifications go
    /// undetected and the daemon keeps answering from a stale graph, which is the cache
    /// inconsistency the missed-events handling exists to close.
    #[test]
    fn rescan_event_with_no_paths_sets_missed_events() {
        let (root, cells, ignores, _t) = fixture();
        let mut state = NotifyFileData::new();
        let event = notify::Event::new(EventKind::Any).set_flag(Flag::Rescan);
        state.process(Ok(event), &root, &cells, &ignores).unwrap();
        assert!(
            state.missed_events,
            "a pathless rescan event must set missed_events"
        );
    }

    #[test]
    fn rescan_event_with_paths_sets_missed_events() {
        let (root, cells, ignores, _t) = fixture();
        let mut state = NotifyFileData::new();
        let path = root.resolve(ProjectRelativePath::new("f").unwrap());
        let event = notify::Event::new(EventKind::Create(CreateKind::File))
            .add_path(path.into_abs_path_buf().into_path_buf())
            .set_flag(Flag::Rescan);
        state.process(Ok(event), &root, &cells, &ignores).unwrap();
        assert!(state.missed_events);
    }

    #[test]
    fn sync_reports_missed_events() {
        let mut state = NotifyFileData::new();
        state.missed_events = true;
        assert!(state.sync().missed_events);
        assert!(!NotifyFileData::new().sync().missed_events);
    }

    /// A sync that cannot tell which files changed surfaces the wipe the same way watchman's
    /// fresh-instance path does.
    #[test]
    fn clear_graph_reports_fresh_instance() {
        let stats = clear_graph(
            FileWatcherStats::new(Default::default(), 0),
            "dropped events".to_owned(),
        );
        assert!(stats.fresh_instance);
        let fresh = stats
            .fresh_instance_data
            .expect("fresh instance data populated");
        assert!(fresh.cleared_dice);
        assert!(stats.incomplete_events_reason.is_some());
    }
}
