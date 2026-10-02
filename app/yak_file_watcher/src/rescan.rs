/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! A snapshot of the metadata of the project's files, which the notify watcher keeps current from
//! its events.
//!
//! The operating system reports a renamed or removed directory as one event for the directory.
//! The snapshot names the paths that were under it, so each of them is invalidated.
//!
//! A build that writes many outputs can overflow the event queue of FSEvents or inotify. Dropping
//! the DICE graph after an overflow makes the next build analyze everything again, and on macOS
//! the rewritten outputs overflow the queue again. Comparing two snapshots finds the files that
//! changed since the earlier one, including the changes whose events were dropped, so only those
//! files are invalidated.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::ops::Bound;
use std::path::Path;
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
/// `yak-out`, the directories of version control systems, and the paths the cells ignore. The
/// paths under a directory sort together, after the directory.
pub(crate) struct Snapshot(BTreeMap<ProjectRelativePathBuf, Entry>);

impl Snapshot {
    pub(crate) fn crawl(
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
    ) -> yak_error::Result<Snapshot> {
        let mut snapshot = Snapshot(BTreeMap::new());
        snapshot.crawl_under(ProjectRelativePathBuf::default(), root, cells, ignore_specs)?;
        Ok(snapshot)
    }

    /// Adds the paths under `dir` to the snapshot.
    fn crawl_under(
        &mut self,
        dir: ProjectRelativePathBuf,
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
    ) -> yak_error::Result<()> {
        let mut dirs = vec![dir];
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
                let Some(info) = read_entry(&entry.path())? else {
                    continue;
                };
                if info == Entry::Directory {
                    dirs.push(path.clone());
                }
                self.0.insert(path, info);
            }
        }
        Ok(())
    }

    /// Brings the entries of `paths`, which file system events named, and of the paths under
    /// them up to date. Records a change for each path under one of `paths` whose entry changed,
    /// and returns how many paths it recorded. The events themselves record the changes of
    /// `paths`.
    ///
    /// FSEvents and inotify report a renamed, removed, or replaced directory as one event for the
    /// directory, so the paths under it are compared with the files.
    pub(crate) fn apply(
        &mut self,
        paths: impl IntoIterator<Item = ProjectRelativePathBuf>,
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &StdYakHashMap<CellName, IgnoreSet>,
        changed: &mut FileChangeTracker,
        stats: &mut FileWatcherStats,
    ) -> yak_error::Result<usize> {
        let mut count = 0;
        for path in paths.into_iter().collect::<HashSet<_>>() {
            if path.is_empty() || is_skipped(&path, cells, ignore_specs) {
                continue;
            }
            let old = Snapshot(self.remove_under(&path).into_iter().collect());
            let mut new = Snapshot(BTreeMap::new());
            let current = read_entry(root.resolve(&path).as_path())?;
            if current == Some(Entry::Directory) {
                new.crawl_under(path.clone(), root, cells, ignore_specs)?;
            }
            count += old.changes(&new, cells, changed, stats);
            self.0.extend(new.0);
            match current {
                Some(entry) => self.0.insert(path, entry),
                None => self.0.remove(&path),
            };
        }
        Ok(count)
    }

    /// The entries under `dir`, without `dir` itself.
    fn under<'a>(
        &'a self,
        dir: &'a ProjectRelativePath,
    ) -> impl Iterator<Item = (&'a ProjectRelativePathBuf, &'a Entry)> + 'a {
        // The paths whose text starts with the text of `dir` sort together, and the paths under
        // `dir` are among them.
        self.0
            .range::<ProjectRelativePath, _>((Bound::Excluded(dir), Bound::Unbounded))
            .take_while(move |(path, _)| path.as_str().starts_with(dir.as_str()))
            .filter(move |(path, _)| path.starts_with(dir))
    }

    fn remove_under(&mut self, dir: &ProjectRelativePath) -> Vec<(ProjectRelativePathBuf, Entry)> {
        let removed: Vec<ProjectRelativePathBuf> =
            self.under(dir).map(|(path, _)| path.clone()).collect();
        removed
            .into_iter()
            .filter_map(|path| self.0.remove(&path).map(|entry| (path, entry)))
            .collect()
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

/// Reads the entry of a path, or `None` if the path does not exist.
fn read_entry(path: &Path) -> yak_error::Result<Option<Entry>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if is_missing(&e) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let file_type = metadata.file_type();
    Ok(Some(if file_type.is_dir() {
        Entry::Directory
    } else if file_type.is_symlink() {
        match fs::read_link(path) {
            Ok(target) => Entry::Symlink(target),
            Err(e) if is_missing(&e) => return Ok(None),
            Err(e) => return Err(e.into()),
        }
    } else {
        Entry::File {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            status_change: StatusChange::of(&metadata),
        }
    }))
}

/// Whether an error means the path does not exist. A path whose parent became a file reports
/// `NotADirectory`.
fn is_missing(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
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

    fn fixture() -> (ProjectRoot, CellResolver, tempfile::TempDir) {
        let tempdir = tempfile::tempdir().unwrap();
        let root_path =
            fs_util::canonicalize(AbsNormPathBuf::new(tempdir.path().to_owned()).unwrap()).unwrap();
        let root = ProjectRoot::new(root_path).unwrap();
        let cells = CellResolver::testing_with_name_and_path(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        );
        (root, cells, tempdir)
    }

    fn changes(root: &ProjectRoot, cells: &CellResolver, before: &Snapshot) -> Vec<String> {
        let after = Snapshot::crawl(root, cells, &StdYakHashMap::default()).unwrap();
        let mut changed = FileChangeTracker::new();
        let mut stats = FileWatcherStats::new(Default::default(), 0);
        before.changes(&after, cells, &mut changed, &mut stats);
        events(stats)
    }

    /// Applies the paths that events named, and returns the changes that `apply` recorded.
    fn apply(
        snapshot: &mut Snapshot,
        root: &ProjectRoot,
        cells: &CellResolver,
        paths: &[&str],
    ) -> Vec<String> {
        let mut changed = FileChangeTracker::new();
        let mut stats = FileWatcherStats::new(Default::default(), 0);
        let paths = paths
            .iter()
            .map(|p| ProjectRelativePathBuf::unchecked_new((*p).to_owned()));
        snapshot
            .apply(
                paths,
                root,
                cells,
                &StdYakHashMap::default(),
                &mut changed,
                &mut stats,
            )
            .unwrap();
        events(stats)
    }

    fn events(stats: FileWatcherStats) -> Vec<String> {
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
        let (root, cells, tempdir) = fixture();
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

    #[test]
    fn test_apply_rename_of_directory() {
        let (root, cells, tempdir) = fixture();
        let dir = tempdir.path();
        fs::create_dir_all(dir.join("pkg/sub")).unwrap();
        fs::write(dir.join("pkg/YAK"), "").unwrap();
        fs::write(dir.join("pkg/sub/YAK"), "").unwrap();
        fs::write(dir.join("pkg-sibling"), "").unwrap();
        let mut snapshot = Snapshot::crawl(&root, &cells, &StdYakHashMap::default()).unwrap();

        fs::rename(dir.join("pkg"), dir.join("pkg2")).unwrap();
        assert_eq!(
            apply(&mut snapshot, &root, &cells, &["pkg", "pkg2"]),
            [
                "Create Directory root//pkg2/sub",
                "Create File root//pkg2/YAK",
                "Create File root//pkg2/sub/YAK",
                "Delete Directory root//pkg/sub",
                "Delete File root//pkg/YAK",
                "Delete File root//pkg/sub/YAK",
            ]
        );
        // The snapshot matches the files, so a rescan finds no other change.
        assert!(changes(&root, &cells, &snapshot).is_empty());

        // Renames that end where they began within one sync leave the same files.
        fs::rename(dir.join("pkg2"), dir.join("pkg")).unwrap();
        fs::rename(dir.join("pkg"), dir.join("pkg2")).unwrap();
        assert!(apply(&mut snapshot, &root, &cells, &["pkg", "pkg2"]).is_empty());

        // Another directory in the place of `pkg2` reports the paths of both.
        fs::rename(dir.join("pkg2"), dir.join("old")).unwrap();
        fs::create_dir(dir.join("pkg2")).unwrap();
        fs::write(dir.join("pkg2/other"), "").unwrap();
        assert_eq!(
            apply(&mut snapshot, &root, &cells, &["pkg2", "old"]),
            [
                "Create Directory root//old/sub",
                "Create File root//old/YAK",
                "Create File root//old/sub/YAK",
                "Create File root//pkg2/other",
                "Delete Directory root//pkg2/sub",
                "Delete File root//pkg2/YAK",
                "Delete File root//pkg2/sub/YAK",
            ]
        );
        assert!(changes(&root, &cells, &snapshot).is_empty());
    }

    #[test]
    fn test_apply_removal_and_replacement() {
        let (root, cells, tempdir) = fixture();
        let dir = tempdir.path();
        fs::create_dir_all(dir.join("a/b")).unwrap();
        fs::write(dir.join("a/b/f"), "").unwrap();
        fs::create_dir_all(dir.join("c")).unwrap();
        fs::write(dir.join("c/g"), "").unwrap();
        let mut snapshot = Snapshot::crawl(&root, &cells, &StdYakHashMap::default()).unwrap();

        // A removal reports the paths that were under the directory, and a file in place of a
        // directory reports the same.
        fs::remove_dir_all(dir.join("a")).unwrap();
        fs::remove_dir_all(dir.join("c")).unwrap();
        fs::write(dir.join("c"), "").unwrap();
        assert_eq!(
            apply(&mut snapshot, &root, &cells, &["a", "c"]),
            [
                "Delete Directory root//a/b",
                "Delete File root//a/b/f",
                "Delete File root//c/g",
            ]
        );
        assert!(changes(&root, &cells, &snapshot).is_empty());

        // A path that events name inside a removed directory needs no entry.
        assert!(apply(&mut snapshot, &root, &cells, &["a/b/f"]).is_empty());
    }
}
