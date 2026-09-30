/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! A snapshot of the metadata of the project's files, which the notify watcher compares when the
//! operating system reports that it dropped events.
//!
//! A build that writes many outputs can overflow the event queue of FSEvents or inotify. Dropping
//! the DICE graph after an overflow makes the next build analyze everything again, and on macOS
//! the rewritten outputs overflow the queue again. Comparing two snapshots finds the files that
//! changed since the earlier one, including the changes whose events were dropped, so only those
//! files are invalidated.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

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
use yak_hash::StdYakHashMap;

use crate::stats::FileWatcherStats;

/// Directories of version control systems, whose files no build reads and which can hold many
/// files.
const VCS_DIRS: &[&str] = &[".git", ".hg", ".jj", ".sl"];

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    File {
        len: u64,
        modified: Option<SystemTime>,
        status_change: StatusChange,
    },
    Directory,
    Symlink(PathBuf),
}

impl Entry {
    fn kind(&self) -> FileWatcherKind {
        match self {
            Entry::File { .. } => FileWatcherKind::File,
            Entry::Directory => FileWatcherKind::Directory,
            Entry::Symlink(_) => FileWatcherKind::Symlink,
        }
    }
}

/// Snapshot holds the metadata of every file, directory, and symlink of the project apart from
/// `yak-out`, the directories of version control systems, and the paths the cells ignore.
pub(crate) struct Snapshot(HashMap<ProjectRelativePathBuf, Entry>);

impl Snapshot {
    pub(crate) fn crawl(
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
    ) -> yak_error::Result<Snapshot> {
        let mut snapshot = Snapshot(HashMap::new());
        let mut dirs = vec![ProjectRelativePathBuf::default()];
        while let Some(dir) = dirs.pop() {
            let entries = match fs::read_dir(root.resolve(&dir).as_path()) {
                Ok(entries) => entries,
                // The directory was removed during the crawl, which its parent's entry records.
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for entry in entries {
                let entry = entry?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    // A cell path cannot name the file, so no build reads it.
                    continue;
                };
                let path = dir.join_normalized(ProjectRelativePath::new(&name)?)?;
                if is_skipped(&path, cells, ignore_specs) {
                    continue;
                }
                let metadata = match fs::symlink_metadata(entry.path()) {
                    Ok(metadata) => metadata,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.into()),
                };
                let file_type = metadata.file_type();
                let info = if file_type.is_dir() {
                    dirs.push(path.clone());
                    Entry::Directory
                } else if file_type.is_symlink() {
                    match fs::read_link(entry.path()) {
                        Ok(target) => Entry::Symlink(target),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    Entry::File {
                        len: metadata.len(),
                        modified: metadata.modified().ok(),
                        status_change: StatusChange::of(&metadata),
                    }
                };
                snapshot.0.insert(path, info);
            }
        }
        Ok(snapshot)
    }

    /// Records the changes from this snapshot to `new` in `changed` and `stats`, and returns
    /// how many paths changed.
    pub(crate) fn changes(
        &self,
        new: &Snapshot,
        cells: &CellResolver,
        changed: &mut FileChangeTracker,
        stats: &mut FileWatcherStats,
    ) -> usize {
        let mut count = 0;
        let mut record = |path: &ProjectRelativePath, event, kind| {
            count += 1;
            let cell_path = cells.get_cell_path(path);
            stats.add(cell_path.to_string(), event, kind);
            record_change(changed, cell_path, event, kind);
        };
        for (path, old) in &self.0 {
            match new.0.get(path) {
                Some(current) if current == old => {}
                Some(current) if current.kind() == old.kind() => {
                    if old.kind() != FileWatcherKind::Directory {
                        record(path, FileWatcherEventType::Modify, old.kind());
                    }
                }
                Some(current) => {
                    record(path, FileWatcherEventType::Delete, old.kind());
                    record(path, FileWatcherEventType::Create, current.kind());
                }
                None => record(path, FileWatcherEventType::Delete, old.kind()),
            }
        }
        for (path, current) in &new.0 {
            if !self.0.contains_key(path) {
                record(path, FileWatcherEventType::Create, current.kind());
            }
        }
        count
    }
}

fn record_change(
    changed: &mut FileChangeTracker,
    path: CellPath,
    event: FileWatcherEventType,
    kind: FileWatcherKind,
) {
    match (event, kind) {
        (FileWatcherEventType::Modify, _) => changed.file_contents_changed(path),
        (_, FileWatcherKind::Directory) => changed.dir_added_or_removed(path),
        (_, FileWatcherKind::File | FileWatcherKind::Symlink) => {
            changed.file_added_or_removed(path)
        }
    }
}

fn is_skipped(
    path: &ProjectRelativePath,
    cells: &CellResolver,
    ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
) -> bool {
    if path.starts_with(InvocationPaths::yak_out_dir_prefix())
        || VCS_DIRS
            .iter()
            .any(|dir| path.as_str() == *dir || path.as_str().starts_with(&format!("{dir}/")))
    {
        return true;
    }
    let cell_path = cells.get_cell_path(path);
    ignore_specs
        .get(&cell_path.cell())
        .is_some_and(|ignore| ignore.is_match(cell_path.path()))
}

/// StatusChange identifies the last change to a file's inode. A program can restore a file's
/// modification time, as `tar -x` and `cp -p` do, but not its status change time, and a
/// rename over the file gives it another inode.
#[derive(Debug, PartialEq, Eq)]
struct StatusChange {
    inode: u64,
    seconds: i64,
    nanoseconds: i64,
}

impl StatusChange {
    #[cfg(unix)]
    fn of(metadata: &fs::Metadata) -> StatusChange {
        use std::os::unix::fs::MetadataExt;
        StatusChange {
            inode: metadata.ino(),
            seconds: metadata.ctime(),
            nanoseconds: metadata.ctime_nsec(),
        }
    }

    /// Windows reports no status change time, so a change that restores a file's size and
    /// modification time goes unnoticed there.
    #[cfg(not(unix))]
    fn of(_metadata: &fs::Metadata) -> StatusChange {
        StatusChange {
            inode: 0,
            seconds: 0,
            nanoseconds: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use yak_core::cells::cell_root_path::CellRootPathBuf;
    use yak_fs::fs_util::uncategorized as fs_util;
    use yak_fs::paths::abs_norm_path::AbsNormPathBuf;

    use super::*;

    fn changes(root: &ProjectRoot, cells: &CellResolver, before: &Snapshot) -> Vec<String> {
        let after = Snapshot::crawl(root, cells, &StdYakHashMap::default()).unwrap();
        let mut changed = FileChangeTracker::new();
        let mut stats = FileWatcherStats::new(Default::default(), 0);
        before.changes(&after, cells, &mut changed, &mut stats);
        let mut events: Vec<String> = stats
            .finish()
            .events
            .into_iter()
            .map(|e| {
                format!(
                    "{:?} {:?} {}",
                    FileWatcherEventType::try_from(e.event).unwrap(),
                    FileWatcherKind::try_from(e.kind).unwrap(),
                    e.path
                )
            })
            .collect();
        events.sort();
        events
    }

    #[test]
    fn test_snapshot_changes() {
        let tempdir = tempfile::tempdir().unwrap();
        let root_path =
            fs_util::canonicalize(AbsNormPathBuf::new(tempdir.path().to_owned()).unwrap()).unwrap();
        let root = ProjectRoot::new(root_path).unwrap();
        let cells = CellResolver::testing_with_name_and_path(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        );
        let dir = tempdir.path();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "a").unwrap();
        fs::write(dir.join("src/old.rs"), "a").unwrap();
        fs::write(dir.join("src/restored.rs"), "a").unwrap();
        fs::create_dir_all(dir.join("yak-out/v2")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        let before = Snapshot::crawl(&root, &cells, &StdYakHashMap::default()).unwrap();
        assert!(changes(&root, &cells, &before).is_empty());

        // A copy that keeps the size and restores the modification time.
        let modified = fs::metadata(dir.join("src/restored.rs"))
            .unwrap()
            .modified()
            .unwrap();
        fs::write(dir.join("src/restored.rs"), "b").unwrap();
        fs::File::options()
            .write(true)
            .open(dir.join("src/restored.rs"))
            .unwrap()
            .set_modified(modified)
            .unwrap();
        fs::write(dir.join("src/lib.rs"), "bb").unwrap();
        fs::remove_file(dir.join("src/old.rs")).unwrap();
        fs::create_dir(dir.join("src/new")).unwrap();
        fs::write(dir.join("yak-out/v2/output"), "out").unwrap();
        fs::write(dir.join(".git/index"), "index").unwrap();
        assert_eq!(
            changes(&root, &cells, &before),
            [
                "Create Directory root//src/new",
                "Delete File root//src/old.rs",
                "Modify File root//src/lib.rs",
                "Modify File root//src/restored.rs",
            ]
        );
    }
}
