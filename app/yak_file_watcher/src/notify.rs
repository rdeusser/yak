/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fs;
use std::io;
use std::mem;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;

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
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_data::FileWatcherEventType;
use yak_data::FileWatcherKind;
use yak_error::conversion::from_any_with_tag;
use yak_events::dispatch::span_async;
use yak_fs::paths::abs_norm_path::AbsNormPath;
use yak_fs::paths::abs_norm_path::AbsNormPathBuf;
use yak_hash::StdYakHashMap;

use crate::file_watcher::FileWatcher;
use crate::mergebase::Mergebase;
use crate::rescan::Snapshot;
use crate::rescan::VCS_DIRS;
use crate::stats::FileWatcherStats;

/// How long a sync waits for the event of its marker.
const MARKER_TIMEOUT: Duration = Duration::from_secs(10);

/// The age after which a marker belongs to a daemon that stopped during a sync.
const STALE_MARKER_AGE: Duration = Duration::from_secs(60);

/// The prefix of the names of sync markers.
const MARKER_PREFIX: &str = ".yak-sync-";

/// SyncMarkers are the files that a sync writes and waits to see an event for. FSEvents and
/// inotify deliver the events of a stream in order, so once the event of a marker arrives, the
/// events of every change made before the marker was written have arrived too. Watchman syncs
/// with cookie files the same way.
struct SyncMarkers {
    /// The directory of the markers: the version control directory of the project root, which
    /// keeps them out of the status of the working tree, or the project root.
    dir: AbsNormPathBuf,
    /// `dir` relative to the project root.
    project_dir: ProjectRelativePathBuf,
    /// The prefix of this daemon's markers, which names its pid. Other daemons can watch the same
    /// project.
    own_prefix: String,
    /// The number of the last marker written.
    written: AtomicU64,
    /// The highest number of this daemon's markers whose event arrived.
    seen: tokio::sync::watch::Sender<u64>,
    /// The marker that `start` wrote for the next sync.
    started: Mutex<Option<WrittenMarker>>,
}

/// A marker that an event names.
enum Marker {
    Own(u64),
    Other,
}

impl SyncMarkers {
    fn new(root: &ProjectRoot) -> SyncMarkers {
        let project_dir = VCS_DIRS
            .iter()
            .map(|dir| ProjectRelativePathBuf::unchecked_new((*dir).to_owned()))
            .find(|dir| root.resolve(dir).as_path().is_dir())
            .unwrap_or_default();
        SyncMarkers {
            dir: root.resolve(&project_dir),
            project_dir,
            own_prefix: format!("{MARKER_PREFIX}{}-", std::process::id()),
            written: AtomicU64::new(0),
            seen: tokio::sync::watch::Sender::new(0),
            started: Mutex::new(None),
        }
    }

    /// Deletes the markers that daemons stopped during a sync left behind.
    fn remove_stale(&self) {
        let entries = match fs::read_dir(self.dir.as_path()) {
            Ok(entries) => entries,
            Err(e) => {
                warn!("FileWatcher: Could not list `{}`: {e}", self.dir);
                return;
            }
        };
        for entry in entries.flatten() {
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with(MARKER_PREFIX)
            {
                continue;
            }
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age > STALE_MARKER_AGE);
            if stale {
                if let Err(e) = fs::remove_file(entry.path()) {
                    warn!(
                        "FileWatcher: Could not remove `{}`: {e}",
                        entry.path().display()
                    );
                }
            }
        }
    }

    fn parse(&self, path: &ProjectRelativePath) -> Option<Marker> {
        if path.parent() != Some(self.project_dir.as_ref()) {
            return None;
        }
        let name = path.file_name()?.as_str();
        if !name.starts_with(MARKER_PREFIX) {
            return None;
        }
        match name
            .strip_prefix(&self.own_prefix)
            .and_then(|n| n.parse().ok())
        {
            Some(n) => Some(Marker::Own(n)),
            None => Some(Marker::Other),
        }
    }

    fn arrived(&self, n: u64) {
        self.seen.send_if_modified(|seen| {
            let newer = n > *seen;
            if newer {
                *seen = n;
            }
            newer
        });
    }

    /// Writes the marker of the next sync, so its event can arrive while the command does other
    /// work. A later `start` replaces it with a newer marker.
    fn start(&self) {
        let marker = self.write();
        *self.started.lock().unwrap() = Some(marker);
    }

    /// Waits for the event of the marker `start` wrote, or writes one. Returns why the marker
    /// failed, in which case the events of earlier changes may not have arrived.
    async fn wait(&self) -> Result<(), String> {
        let started = self.started.lock().unwrap().take();
        let marker = started.unwrap_or_else(|| self.write());
        marker.wait().await
    }

    fn write(&self) -> WrittenMarker {
        let n = self.written.fetch_add(1, Ordering::Relaxed) + 1;
        let path = self.dir.as_path().join(format!("{}{n}", self.own_prefix));
        let seen = self.seen.subscribe();
        let written = fs::write(&path, b"").map_err(|e| {
            format!(
                "notify could not write its sync marker `{}`: {e}",
                path.display()
            )
        });
        WrittenMarker {
            n,
            path,
            seen,
            written,
        }
    }
}

/// A marker that a sync waits for. Dropping it deletes the file.
struct WrittenMarker {
    n: u64,
    path: PathBuf,
    seen: tokio::sync::watch::Receiver<u64>,
    written: Result<(), String>,
}

impl WrittenMarker {
    async fn wait(mut self) -> Result<(), String> {
        if let Err(e) = &self.written {
            return Err(e.clone());
        }
        let n = self.n;
        match tokio::time::timeout(MARKER_TIMEOUT, self.seen.wait_for(|seen| *seen >= n)).await {
            Ok(Ok(_)) => Ok(()),
            // The watcher owns the sender, so it outlives the wait.
            Ok(Err(_)) => Err("notify stopped before its sync marker arrived".to_owned()),
            Err(_) => Err(format!(
                "the event of notify's sync marker did not arrive within {} seconds",
                MARKER_TIMEOUT.as_secs()
            )),
        }
    }
}

impl Drop for WrittenMarker {
    fn drop(&mut self) {
        if self.written.is_err() {
            return;
        }
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => warn!(
                "FileWatcher: Could not remove `{}`: {e}",
                self.path.display()
            ),
        }
    }
}

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
        markers: &SyncMarkers,
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

            match markers.parse(&path) {
                Some(Marker::Own(n)) => {
                    markers.arrived(n);
                    continue;
                }
                Some(Marker::Other) => continue,
                None => {}
            }

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
    #[allocative(skip)]
    markers: Arc<SyncMarkers>,
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
        let markers = Arc::new(SyncMarkers::new(root));
        markers.remove_stale();
        let markers2 = markers.dupe();
        let handler = move |event| {
            let mut guard = data2.lock().unwrap();
            if let Ok(state) = &mut *guard {
                if let Err(e) = state.process(event, &root2, &cells2, &ignore_specs2, &markers2) {
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
            markers,
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
        let marker = self.markers.wait().await;
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
        let missed = if missed_events {
            Some("notify dropped events".to_owned())
        } else {
            marker.err()
        };

        // A failure below leaves the state lost.
        let previous = mem::replace(&mut *self.snapshot.lock().unwrap(), SnapshotState::Lost);
        let (next, clear_reason) = match (previous, missed) {
            (SnapshotState::Current(snapshot), None) => {
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
            // Events were dropped or may not have arrived, so the files that changed since the
            // last sync are found by crawling again.
            (SnapshotState::Current(previous), Some(missed)) => match self.crawl().await {
                Ok(snapshot) => {
                    let count = previous.changes(&snapshot, &self.cells, &mut changed, &mut stats);
                    stats.base_mut().incomplete_events_reason = Some(format!(
                        "{missed}, and a rescan of the project found {count} changed paths"
                    ));
                    (SnapshotState::Current(snapshot), None)
                }
                Err(e) => {
                    warn!("FileWatcher: Could not crawl the project: {e:#}");
                    (
                        SnapshotState::Lost,
                        Some(format!(
                            "{missed}, and a rescan of the project failed: {e:#}"
                        )),
                    )
                }
            },
            // The first sync takes the snapshot that later syncs keep current.
            (SnapshotState::Initial, missed) => {
                let next = match self.crawl().await {
                    Ok(snapshot) => SnapshotState::Current(snapshot),
                    Err(e) => {
                        warn!("FileWatcher: Could not crawl the project: {e:#}");
                        SnapshotState::Lost
                    }
                };
                (next, missed)
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
    fn start_sync(&self) {
        self.markers.start();
    }

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
        state
            .process(Ok(event), &root, &cells, &ignores, &SyncMarkers::new(&root))
            .unwrap();
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
        state
            .process(Ok(event), &root, &cells, &ignores, &SyncMarkers::new(&root))
            .unwrap();
        assert!(state.missed_events);
    }

    #[test]
    fn sync_reports_missed_events() {
        let mut state = NotifyFileData::new();
        state.missed_events = true;
        assert!(state.sync().missed_events);
        assert!(!NotifyFileData::new().sync().missed_events);
    }

    /// A change made just before a sync is among the sync's events. FSEvents delivers events
    /// asynchronously, so a sync that takes the events at once misses some of them.
    #[tokio::test]
    async fn sync_marker_waits_for_earlier_changes() {
        let (root, cells, ignores, tempdir) = fixture();
        let watcher = NotifyFileWatcher::new(&root, cells, ignores).unwrap();
        for i in 0..200 {
            let name = format!("f{i}");
            fs::write(tempdir.path().join(&name), "").unwrap();
            // The command's update starts the sync for half of the syncs.
            if i % 2 == 0 {
                watcher.start_sync();
            }
            watcher.markers.wait().await.unwrap();
            let data = mem::replace(
                &mut *watcher.data.lock().unwrap(),
                Ok(NotifyFileData::new()),
            )
            .unwrap();
            let paths: Vec<&str> = data.paths.iter().map(|p| p.as_str()).collect();
            assert!(
                paths.contains(&name.as_str()),
                "sync {i} missed `{name}`: {paths:?}"
            );
            assert!(
                paths.iter().all(|p| !p.contains(MARKER_PREFIX)),
                "a marker reached the events: {paths:?}"
            );
        }
        assert_eq!(markers_in(tempdir.path()), Vec::<String>::new());
    }

    fn markers_in(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(MARKER_PREFIX))
            .collect();
        names.sort();
        names
    }

    /// Markers go in the version control directory. A new watcher deletes the markers that a
    /// stopped daemon left behind, and a sync ignores the markers of other daemons and deletes a
    /// marker that a newer one replaced.
    #[tokio::test]
    async fn sync_markers_in_vcs_dir() {
        let (root, cells, ignores, tempdir) = fixture();
        let git = tempdir.path().join(".git");
        fs::create_dir(&git).unwrap();
        let stale = fs::File::create(git.join(format!("{MARKER_PREFIX}2-1"))).unwrap();
        stale
            .set_modified(SystemTime::now() - 2 * STALE_MARKER_AGE)
            .unwrap();
        let watcher = NotifyFileWatcher::new(&root, cells, ignores).unwrap();
        assert_eq!(markers_in(&git), Vec::<String>::new());
        assert_eq!(
            watcher.markers.dir.as_path(),
            root.root().as_path().join(".git")
        );

        let other = format!("{MARKER_PREFIX}1-1");
        fs::write(git.join(&other), "").unwrap();
        watcher.start_sync();
        watcher.start_sync();
        assert_eq!(markers_in(&git).len(), 2, "{:?}", markers_in(&git));
        watcher.markers.wait().await.unwrap();
        assert_eq!(markers_in(&git), vec![other]);
        let data = mem::replace(
            &mut *watcher.data.lock().unwrap(),
            Ok(NotifyFileData::new()),
        )
        .unwrap();
        assert!(data.paths.is_empty(), "{:?}", data.paths);
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
