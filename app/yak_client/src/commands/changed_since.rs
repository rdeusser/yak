/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Lists the files that changed since a Git revision, for `--changed-since`.

use std::process::Command;
use std::process::Output;

use async_trait::async_trait;
use dupe::Dupe;
use yak_cli_proto::ChangedFiles;
use yak_common::invocation_paths::InvocationPaths;
use yak_common::legacy_configs::cells::YakConfigBasedCells;
use yak_common::legacy_configs::file_ops::ConfigDirEntry;
use yak_common::legacy_configs::file_ops::ConfigParserFileOps;
use yak_common::legacy_configs::file_ops::ConfigPath;
use yak_common::legacy_configs::file_ops::DefaultConfigParserFileOps;
use yak_core::fs::project::ProjectRoot;
use yak_fs::paths::file_name::FileNameBuf;

#[derive(Debug, yak_error::Error)]
#[yak(tag = Input)]
enum ChangedSinceError {
    #[error("Could not run `git`: {0}")]
    GitNotRun(std::io::Error),
    #[error("`git {args}` failed: {stderr}")]
    GitFailed { args: String, stderr: String },
    #[error("`git {0}` printed output that is not UTF-8")]
    NotUtf8(String),
    #[error("Unexpected `git diff --raw` output `{0}`")]
    BadRawDiff(String),
    #[error("Unexpected `git ls-tree` output `{0}`")]
    BadTreeListing(String),
}

/// `Git` runs Git in the project root, so the paths that Git takes and prints are relative to the
/// project root.
struct Git {
    root: ProjectRoot,
}

impl Git {
    fn output(&self, args: &[&str]) -> yak_error::Result<Output> {
        Ok(Command::new("git")
            .args(args)
            .current_dir(self.root.root().as_path())
            .output()
            .map_err(ChangedSinceError::GitNotRun)?)
    }

    fn run(&self, args: &[&str]) -> yak_error::Result<Vec<u8>> {
        let output = self.output(args)?;
        if !output.status.success() {
            return Err(ChangedSinceError::GitFailed {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }
            .into());
        }
        Ok(output.stdout)
    }

    fn run_utf8(&self, args: &[&str]) -> yak_error::Result<String> {
        String::from_utf8(self.run(args)?)
            .map_err(|_| ChangedSinceError::NotUtf8(args.join(" ")).into())
    }

    fn is_ignored(&self, path: &str) -> yak_error::Result<bool> {
        let args = ["check-ignore", "--quiet", "--", path];
        let output = self.output(&args)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(ChangedSinceError::GitFailed {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }
            .into()),
        }
    }
}

/// Lists the files that differ between the working tree and the merge base of `revision` and
/// `HEAD`, including untracked files that Git does not ignore, and compares the project
/// configuration at the merge base with the current one.
pub(crate) async fn changed_since(
    root: &ProjectRoot,
    revision: &str,
) -> yak_error::Result<ChangedFiles> {
    let git = Git { root: root.dupe() };
    let commit = git.run_utf8(&[
        "rev-parse",
        "--verify",
        "--end-of-options",
        &format!("{revision}^{{commit}}"),
    ])?;
    let merge_base = git.run_utf8(&["merge-base", commit.trim(), "HEAD"])?;
    let merge_base = merge_base.trim().to_owned();

    let raw = git.run(&[
        "diff",
        "--raw",
        "-z",
        "--no-renames",
        "--ignore-submodules=none",
        "--relative",
        &merge_base,
        "--",
    ])?;
    let mut changes = parse_raw_diff(&raw)?;
    let untracked = git.run(&["ls-files", "--others", "--exclude-standard", "-z"])?;
    for path in split_nul(&untracked, "ls-files")? {
        changes.added.push(path.to_owned());
    }
    // yak writes `yak-out` and never reads a source file from it.
    let yak_out = InvocationPaths::yak_out_dir_prefix().as_str();
    let outside_yak_out =
        |path: &String| path != yak_out && !path.starts_with(&format!("{yak_out}/"));
    changes.added.retain(outside_yak_out);
    changes.removed.retain(outside_yak_out);
    changes.modified.retain(outside_yak_out);

    changes.config_difference = YakConfigBasedCells::describe_difference(
        &mut DefaultConfigParserFileOps::new(root.dupe()),
        &mut GitConfigFileOps {
            git: &git,
            commit: &merge_base,
            disk: DefaultConfigParserFileOps::new(root.dupe()),
        },
    )
    .await?;
    changes.revision = revision.to_owned();
    changes.merge_base = merge_base;
    Ok(changes)
}

fn split_nul<'a>(output: &'a [u8], command: &str) -> yak_error::Result<Vec<&'a str>> {
    Ok(std::str::from_utf8(output)
        .map_err(|_| ChangedSinceError::NotUtf8(command.to_owned()))?
        .split('\0')
        .filter(|s| !s.is_empty())
        .collect())
}

/// Parses the output of `git diff --raw -z --no-renames`. Each entry is a line of modes, object
/// IDs, and a status letter, then a path. A change between a file and a symbolic link counts as a
/// removal and an addition.
fn parse_raw_diff(raw: &[u8]) -> yak_error::Result<ChangedFiles> {
    let mut changes = ChangedFiles::default();
    let fields = split_nul(raw, "diff")?;
    for entry in fields.chunks(2) {
        let [meta, path] = entry else {
            return Err(ChangedSinceError::BadRawDiff(entry.join(" ")).into());
        };
        let words: Vec<&str> = meta.split(' ').collect();
        let [src_mode, dst_mode, _, _, status] = words.as_slice() else {
            return Err(ChangedSinceError::BadRawDiff((*meta).to_owned()).into());
        };
        let path = (*path).to_owned();
        const GITLINK: &str = "160000";
        if src_mode.trim_start_matches(':') == GITLINK || *dst_mode == GITLINK {
            changes.submodules.push(path);
            continue;
        }
        match status.chars().next() {
            Some('A') => changes.added.push(path),
            Some('D') => changes.removed.push(path),
            Some('T') => {
                changes.removed.push(path.clone());
                changes.added.push(path);
            }
            Some(_) => changes.modified.push(path),
            None => return Err(ChangedSinceError::BadRawDiff((*meta).to_owned()).into()),
        }
    }
    Ok(changes)
}

/// `GitConfigFileOps` reads the configuration files of a commit. A file that the commit does not
/// have and that Git ignores, such as `.yakconfig.local`, belongs to the machine and is read from
/// the file system, as the current configuration reads it.
struct GitConfigFileOps<'a> {
    git: &'a Git,
    commit: &'a str,
    disk: DefaultConfigParserFileOps,
}

/// `TreeEntry` is one line of `git ls-tree` output.
struct TreeEntry<'a> {
    is_tree: bool,
    is_blob: bool,
    object: &'a str,
    path: &'a str,
}

fn parse_tree_entries(output: &[u8]) -> yak_error::Result<Vec<TreeEntry<'_>>> {
    split_nul(output, "ls-tree")?
        .into_iter()
        .map(|line| {
            let (meta, path) = line
                .split_once('\t')
                .ok_or_else(|| ChangedSinceError::BadTreeListing(line.to_owned()))?;
            let words: Vec<&str> = meta.split(' ').collect();
            let [_, kind, object] = words.as_slice() else {
                return Err(ChangedSinceError::BadTreeListing(line.to_owned()).into());
            };
            Ok(TreeEntry {
                is_tree: *kind == "tree",
                is_blob: *kind == "blob",
                object,
                path,
            })
        })
        .collect()
}

#[async_trait]
impl ConfigParserFileOps for GitConfigFileOps<'_> {
    async fn read_file_lines_if_exists(
        &mut self,
        path: &ConfigPath,
    ) -> yak_error::Result<Option<Vec<String>>> {
        let ConfigPath::Project(project_path) = path else {
            return self.disk.read_file_lines_if_exists(path).await;
        };
        let listing = self
            .git
            .run(&["ls-tree", "-z", self.commit, "--", project_path.as_str()])?;
        let entries = parse_tree_entries(&listing)?;
        match entries.first() {
            Some(entry) if entry.is_blob => {
                let contents = self.git.run_utf8(&["cat-file", "blob", entry.object])?;
                Ok(Some(contents.lines().map(ToOwned::to_owned).collect()))
            }
            Some(_) => Ok(None),
            None if self.git.is_ignored(project_path.as_str())? => {
                self.disk.read_file_lines_if_exists(path).await
            }
            None => Ok(None),
        }
    }

    async fn read_dir(&mut self, path: &ConfigPath) -> yak_error::Result<Vec<ConfigDirEntry>> {
        let ConfigPath::Project(project_path) = path else {
            return self.disk.read_dir(path).await;
        };
        let dir = format!("{}/", project_path.as_str());
        let listing = self.git.run(&["ls-tree", "-z", self.commit, "--", &dir])?;
        let mut names = Vec::new();
        let mut entries = Vec::new();
        for entry in parse_tree_entries(&listing)? {
            let name = entry
                .path
                .rsplit('/')
                .next()
                .unwrap_or(entry.path)
                .to_owned();
            names.push(name.clone());
            entries.push(ConfigDirEntry::new(
                FileNameBuf::try_from(name)?,
                entry.is_tree,
            ));
        }
        for entry in self.disk.read_dir(path).await? {
            let name = entry.name().as_str();
            if !names.iter().any(|n| n == name) && self.git.is_ignored(&format!("{dir}{name}"))? {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_raw_diff_sorts_entries_by_status() {
        let raw = [
            ":000000 100644 0000000 1111111 A\0new.rs\0",
            ":100644 000000 2222222 0000000 D\0old.rs\0",
            ":100644 100644 3333333 4444444 M\0YAK\0",
            ":100644 120000 5555555 6666666 T\0link\0",
            ":160000 160000 7777777 8888888 M\0vendor/sub\0",
        ]
        .concat();
        let changes = parse_raw_diff(raw.as_bytes()).unwrap();
        assert_eq!(vec!["new.rs", "link"], changes.added);
        assert_eq!(vec!["old.rs", "link"], changes.removed);
        assert_eq!(vec!["YAK"], changes.modified);
        assert_eq!(vec!["vendor/sub"], changes.submodules);
    }

    #[test]
    fn parse_raw_diff_rejects_a_path_without_status() {
        assert!(parse_raw_diff(b":100644 100644 1 2 M\0").is_err());
    }
}
