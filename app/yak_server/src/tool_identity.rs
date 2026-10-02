/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The identities of the tools that the system toolchains run from `PATH`, which the daemon
//! offers as `tool_identity.<tool>` in the configuration of each command. A toolchain puts the
//! identity of its tool into the keys of its actions, so an action runs again when its compiler
//! changes, and a remote cache keeps the results of different compilers apart.
//!
//! An identity is a digest of the output of the tool's version command. [`ToolIdentities`] keeps
//! each identity with the metadata of the files that decide that output, and a command runs the
//! version command again only when the metadata of one of those files changed.

use std::fs::Metadata;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use futures::future::join_all;
use parking_lot::Mutex;
use yak_cli_proto::ConfigOverride;
use yak_cli_proto::config_override::ConfigType;
use yak_common::legacy_configs::args::ComputedConfigValue;

/// Tool is a tool that `system_toolchains()` runs from `PATH`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Rustc,
    Go,
    Clang,
}

const TOOLS: [Tool; 3] = [Tool::Rustc, Tool::Go, Tool::Clang];

/// TIMEOUT limits the run of one command.
const TIMEOUT: Duration = Duration::from_secs(10);

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Rustc => "rustc",
            Tool::Go => "go",
            Tool::Clang => "clang",
        }
    }

    fn version_args(self) -> &'static [&'static str] {
        match self {
            Tool::Rustc => &["-vV"],
            Tool::Go => &["version"],
            Tool::Clang => &["--version"],
        }
    }

    /// selection_files returns the files besides the tool's own that select what its version
    /// command runs. `canonical` is the canonical path of the tool's file on `PATH`.
    fn selection_files(self, env: &ToolEnv, project_root: &Path, canonical: &Path) -> Vec<PathBuf> {
        match self {
            Tool::Rustc => {
                // rustup reads the nearest `rust-toolchain.toml` or `rust-toolchain`, then the
                // directory overrides and the default toolchain of `settings.toml`.
                let mut files: Vec<PathBuf> = project_root
                    .ancestors()
                    .flat_map(|dir| [dir.join("rust-toolchain.toml"), dir.join("rust-toolchain")])
                    .collect();
                files.extend(
                    env.rustup_home
                        .as_ref()
                        .map(|home| home.join("settings.toml")),
                );
                files
            }
            Tool::Go => {
                // `go` switches to the toolchain that the nearest `go.work` or `go.mod` names
                // when `GOTOOLCHAIN` allows it, which the `GOENV` file or `$GOROOT/go.env` can
                // set. `canonical` is `$GOROOT/bin/go`.
                let mut files: Vec<PathBuf> = project_root
                    .ancestors()
                    .flat_map(|dir| [dir.join("go.mod"), dir.join("go.work")])
                    .collect();
                files.extend(env.goenv.clone());
                files.extend(
                    canonical
                        .parent()
                        .and_then(Path::parent)
                        .map(|goroot| goroot.join("go.env")),
                );
                files
            }
            Tool::Clang => {
                if cfg!(target_os = "macos") {
                    // `xcode-select --switch` replaces this link.
                    vec![PathBuf::from("/var/db/xcode_select_link")]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// locate returns the command whose output names the files that the tool's file on `PATH`
    /// runs, such as the compiler of the toolchain that a rustup proxy selects.
    fn locate(self, found: &Path, canonical: &Path) -> Option<(PathBuf, &'static [&'static str])> {
        match self {
            Tool::Rustc => Some((found.to_owned(), &["--print", "sysroot"])),
            Tool::Go => None,
            // The files of `/usr/bin` run the clang of the selected Xcode or Command Line Tools.
            Tool::Clang => (cfg!(target_os = "macos") && canonical.starts_with("/usr/bin"))
                .then(|| (PathBuf::from("/usr/bin/xcrun"), &["--find", "clang"][..])),
        }
    }

    /// located_files returns the files that the output of the `locate` command names, or `None`
    /// if the output names no absolute path.
    fn located_files(self, stdout: &[u8]) -> Option<Vec<PathBuf>> {
        let path = PathBuf::from(std::str::from_utf8(stdout).ok()?.trim());
        if !path.is_absolute() {
            return None;
        }
        Some(match self {
            Tool::Rustc => vec![
                path.join("bin")
                    .join(format!("rustc{}", std::env::consts::EXE_SUFFIX)),
                path.join("lib/rustlib/multirust-channel-manifest.toml"),
            ],
            Tool::Go => Vec::new(),
            Tool::Clang => vec![path],
        })
    }

    /// is_set_by reports whether a `-c tool_identity.<tool>=...` of the command line sets or
    /// unsets the identity in the root cell, which hides a computed value.
    fn is_set_by(self, config_overrides: &[ConfigOverride]) -> bool {
        let key = format!("tool_identity.{}", self.name());
        config_overrides.iter().any(|o| {
            o.config_type == ConfigType::Value as i32
                && o.cell.as_deref().is_none_or(str::is_empty)
                && o.config_override
                    .split_once('=')
                    .is_some_and(|(k, _)| k == key)
        })
    }
}

/// ToolEnv is the part of the daemon's environment that selects the tools. The environment stays
/// fixed for the daemon's lifetime.
struct ToolEnv {
    path: Vec<PathBuf>,
    rustup_home: Option<PathBuf>,
    /// The file of `go env -w`, which `GOENV=off` turns off.
    goenv: Option<PathBuf>,
}

impl ToolEnv {
    fn from_process() -> Self {
        let path = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        let rustup_home = std::env::var_os("RUSTUP_HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".rustup")));
        let goenv = match std::env::var_os("GOENV") {
            Some(goenv) if goenv == "off" => None,
            Some(goenv) if !goenv.is_empty() => Some(PathBuf::from(goenv)),
            _ => dirs::config_dir().map(|dir| dir.join("go").join("env")),
        };
        Self {
            path,
            rustup_home,
            goenv,
        }
    }
}

/// Stamp is the metadata of a file that changes when the file is written or replaced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device_and_inode: (u64, u64),
    #[cfg(unix)]
    changed: (i64, i64),
    #[cfg(unix)]
    mode: u32,
}

impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device_and_inode: (metadata.dev(), metadata.ino()),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
            #[cfg(unix)]
            mode: metadata.mode(),
        }
    }
}

/// Stamps holds the stamp of each file that decides the output of a version command, or `None`
/// for a file that is missing. A link has the stamp of the link.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Stamps(Vec<(PathBuf, Option<Stamp>)>);

impl Stamps {
    fn push(&mut self, path: PathBuf) {
        let stamp = std::fs::symlink_metadata(&path)
            .ok()
            .map(|metadata| Stamp::of(&metadata));
        self.0.push((path, stamp));
    }
}

/// Found is the tool's file on `PATH`.
struct Found {
    path: PathBuf,
    canonical: PathBuf,
}

/// Resolved is the tool's file on `PATH`, with the stamps of the files that select it and of the
/// files that select what it runs.
struct Resolved {
    found: Option<Found>,
    stamps: Stamps,
}

impl Resolved {
    fn new(tool: Tool, env: &ToolEnv, project_root: &Path) -> Self {
        let file_name = format!("{}{}", tool.name(), std::env::consts::EXE_SUFFIX);
        let mut stamps = Stamps::default();
        let mut found = None;
        // The first executable file on `PATH` is the one that a spawn of the name runs. The
        // directories before it get the stamp of a missing file, so a file added there counts as
        // a change.
        for dir in &env.path {
            let candidate = dir.join(&file_name);
            stamps.push(candidate.clone());
            if is_executable_file(&candidate) {
                let canonical = std::fs::canonicalize(&candidate).unwrap_or(candidate.clone());
                found = Some(Found {
                    path: candidate,
                    canonical,
                });
                break;
            }
        }
        if let Some(found) = &found {
            // The canonical path changes when a link on the way to it is switched, such as a
            // Homebrew `opt` directory.
            stamps.push(found.canonical.clone());
            for file in tool.selection_files(env, project_root, &found.canonical) {
                stamps.push(file);
            }
        }
        Self { found, stamps }
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// Cached is an identity with the stamps of the files it came from, taken before the commands
/// that produced it ran.
struct Cached {
    stamps: Stamps,
    /// The files that the `locate` command named.
    located: Vec<PathBuf>,
    identity: String,
}

/// ToolIdentities computes the identities of the tools for each command of a daemon. It keeps
/// each identity while the stamps of its files stay the same.
pub(crate) struct ToolIdentities {
    env: ToolEnv,
    project_root: PathBuf,
    /// The cache of each tool of `TOOLS`, in its order.
    cached: [Mutex<Option<Arc<Cached>>>; TOOLS.len()],
}

impl ToolIdentities {
    /// new returns the identities of the tools that the daemon's `PATH` names, whose version
    /// commands run in `project_root`, where `rustup` reads `rust-toolchain.toml`.
    pub(crate) fn new(project_root: &Path) -> Self {
        Self::with_env(ToolEnv::from_process(), project_root)
    }

    fn with_env(env: ToolEnv, project_root: &Path) -> Self {
        Self {
            env,
            project_root: project_root.to_owned(),
            cached: Default::default(),
        }
    }

    /// identities returns `tool_identity.<tool>` for each tool that `config_overrides` leaves
    /// unset. A `-c` of the command line overrides the computed values.
    pub(crate) async fn identities(
        &self,
        config_overrides: &[ConfigOverride],
    ) -> Vec<ComputedConfigValue> {
        join_all(
            TOOLS
                .iter()
                .enumerate()
                .filter(|(_, tool)| !tool.is_set_by(config_overrides))
                .map(|(index, &tool)| async move {
                    ComputedConfigValue {
                        section: "tool_identity".to_owned(),
                        key: tool.name().to_owned(),
                        value: self.identity(index, tool).await,
                    }
                }),
        )
        .await
    }

    async fn identity(&self, index: usize, tool: Tool) -> String {
        let cached = self.cached[index].lock().clone();
        let resolved = Resolved::new(tool, &self.env, &self.project_root);
        if let Some(cached) = &cached {
            let mut stamps = resolved.stamps.clone();
            for file in &cached.located {
                stamps.push(file.clone());
            }
            if stamps == cached.stamps {
                return cached.identity.clone();
            }
        }

        let (identity, cached) = compute(tool, resolved, &self.project_root).await;
        if let Some(cached) = cached {
            *self.cached[index].lock() = Some(Arc::new(cached));
        }
        identity
    }
}

/// compute runs the version command of `tool` and returns its identity, with the cache entry
/// for it unless a change could go unseen. `resolved` was stamped before any command ran, and
/// the located files are stamped before the version command runs, so a change during a run
/// makes the next command run it again.
async fn compute(tool: Tool, resolved: Resolved, cwd: &Path) -> (String, Option<Cached>) {
    let Resolved { found, mut stamps } = resolved;
    let Some(found) = found else {
        let identity = identity(&VersionOutput::NotStarted(std::io::ErrorKind::NotFound));
        let cached = Cached {
            stamps,
            located: Vec::new(),
            identity: identity.clone(),
        };
        return (identity, Some(cached));
    };

    let mut located = Vec::new();
    let mut complete = true;
    if let Some((program, args)) = tool.locate(&found.path, &found.canonical) {
        tracing::debug!(
            "Locating the files of {} with {}",
            tool.name(),
            program.display()
        );
        match run(&program, args, cwd).await {
            VersionOutput::Exited {
                code: Some(0),
                stdout,
                ..
            } => match tool.located_files(&stdout) {
                Some(files) => located = files,
                None => complete = false,
            },
            _ => complete = false,
        }
        for file in &located {
            stamps.push(file.clone());
        }
    }

    tracing::debug!("Computing the identity of {}", tool.name());
    let output = run(&found.path, tool.version_args(), cwd).await;
    let identity = identity(&output);
    // A failed spawn or a timeout can pass, and a failed `locate` leaves files unstamped.
    let cached = (complete && matches!(output, VersionOutput::Exited { .. })).then(|| Cached {
        stamps,
        located,
        identity: identity.clone(),
    });
    (identity, cached)
}

/// VersionOutput is what a version command produced.
#[derive(Debug)]
enum VersionOutput {
    Exited {
        code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// The command did not start, such as when the tool is not on `PATH`.
    NotStarted(std::io::ErrorKind),
    TimedOut,
}

async fn run(program: &Path, args: &[&str], cwd: &Path) -> VersionOutput {
    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(TIMEOUT, output).await {
        Ok(Ok(output)) => VersionOutput::Exited {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        },
        Ok(Err(e)) => VersionOutput::NotStarted(e.kind()),
        Err(_) => VersionOutput::TimedOut,
    }
}

/// identity returns a digest of `output`, which is stable across machines and yak versions for
/// the same output, including the output of a missing tool.
fn identity(output: &VersionOutput) -> String {
    let mut hasher = blake3::Hasher::new();
    match output {
        VersionOutput::Exited {
            code,
            stdout,
            stderr,
        } => {
            hasher.update(format!("exited {code:?}\0").as_bytes());
            hasher.update(stdout);
            hasher.update(b"\0");
            hasher.update(stderr);
        }
        VersionOutput::NotStarted(kind) => {
            hasher.update(format!("not started {kind:?}").as_bytes());
        }
        VersionOutput::TimedOut => {
            hasher.update(b"timed out");
        }
    }
    hasher.finalize().to_hex()[..16].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_follow_the_output() {
        let exited = |stdout: &[u8]| VersionOutput::Exited {
            code: Some(0),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(
            identity(&exited(b"rustc 1.90.0")),
            identity(&exited(b"rustc 1.90.0"))
        );
        assert_ne!(
            identity(&exited(b"rustc 1.90.0")),
            identity(&exited(b"rustc 1.91.0"))
        );
        assert_ne!(
            identity(&VersionOutput::NotStarted(std::io::ErrorKind::NotFound)),
            identity(&exited(b""))
        );
        assert_eq!(identity(&exited(b"")).len(), 16);
    }

    #[test]
    fn a_flag_of_the_root_cell_sets_an_identity() {
        let value = |cell: Option<&str>, arg: &str| ConfigOverride {
            cell: cell.map(str::to_owned),
            config_override: arg.to_owned(),
            config_type: ConfigType::Value as i32,
        };
        assert!(Tool::Rustc.is_set_by(&[value(None, "tool_identity.rustc=pinned")]));
        assert!(Tool::Rustc.is_set_by(&[value(Some(""), "tool_identity.rustc=")]));
        assert!(!Tool::Go.is_set_by(&[value(None, "tool_identity.rustc=pinned")]));
        assert!(!Tool::Rustc.is_set_by(&[value(Some("other"), "tool_identity.rustc=pinned")]));
        assert!(!Tool::Rustc.is_set_by(&[ConfigOverride {
            cell: None,
            config_override: "/tmp/tool_identity.rustc=pinned".to_owned(),
            config_type: ConfigType::File as i32,
        }]));
    }

    #[cfg(unix)]
    mod fake_tools {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::fs::symlink;

        use super::*;

        /// Workspace is a temporary `PATH`, project, and rustup home with fake tools, whose runs
        /// a log records.
        struct Workspace {
            dir: tempfile::TempDir,
        }

        impl Workspace {
            fn new() -> Self {
                let workspace = Self {
                    dir: tempfile::tempdir().unwrap(),
                };
                for dir in ["bin", "project", "rustup"] {
                    std::fs::create_dir(workspace.path(dir)).unwrap();
                }
                workspace
            }

            fn path(&self, relative: &str) -> PathBuf {
                self.dir.path().join(relative)
            }

            fn identities(&self, path: &[&str]) -> ToolIdentities {
                let env = ToolEnv {
                    path: path.iter().map(|dir| self.path(dir)).collect(),
                    rustup_home: Some(self.path("rustup")),
                    goenv: Some(self.path("goenv")),
                };
                ToolIdentities::with_env(env, &self.path("project"))
            }

            /// write_tool replaces the file at `relative` with a script that logs its arguments
            /// and prints `version`, as an installer replaces a file.
            fn write_tool(&self, relative: &str, version: &str) {
                self.write_script(
                    relative,
                    &format!(
                        "echo \"$0 $*\" >> '{log}'\necho '{version}'\n",
                        log = self.path("runs.log").display(),
                    ),
                );
            }

            /// write_rustup_proxy writes a rustc on `PATH` that prints the sysroot `sysroot`
            /// and runs the rustc there, as a rustup proxy does.
            fn write_rustup_proxy(&self, relative: &str, sysroot: &str) {
                let sysroot = self.path(sysroot);
                self.write_script(
                    relative,
                    &format!(
                        "if [ \"$1\" = --print ]; then echo '{sysroot}'; exit 0; fi\nexec '{sysroot}/bin/rustc' \"$@\"\n",
                        sysroot = sysroot.display(),
                    ),
                );
            }

            fn write_script(&self, relative: &str, body: &str) {
                let path = self.path(relative);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                let temporary = path.with_extension("new");
                std::fs::write(&temporary, format!("#!/bin/sh\n{body}")).unwrap();
                std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
                std::fs::rename(&temporary, &path).unwrap();
            }

            fn runs(&self) -> usize {
                std::fs::read_to_string(self.path("runs.log"))
                    .map(|log| log.lines().count())
                    .unwrap_or(0)
            }
        }

        async fn identity_of(identities: &ToolIdentities, tool: Tool) -> String {
            let index = TOOLS.iter().position(|t| *t == tool).unwrap();
            identities.identity(index, tool).await
        }

        #[tokio::test]
        async fn an_unchanged_tool_runs_once() {
            let workspace = Workspace::new();
            workspace.write_tool("bin/go", "go1");
            let identities = workspace.identities(&["bin"]);

            let first = identity_of(&identities, Tool::Go).await;
            assert_eq!(identity_of(&identities, Tool::Go).await, first);
            assert_eq!(workspace.runs(), 1);
        }

        #[tokio::test]
        async fn a_replaced_tool_runs_again() {
            let workspace = Workspace::new();
            workspace.write_tool("bin/go", "go1");
            let identities = workspace.identities(&["bin"]);
            let first = identity_of(&identities, Tool::Go).await;

            workspace.write_tool("bin/go", "go2");
            assert_ne!(identity_of(&identities, Tool::Go).await, first);
            assert_eq!(workspace.runs(), 2);
        }

        #[tokio::test]
        async fn a_switched_link_runs_again() {
            let workspace = Workspace::new();
            workspace.write_tool("cellar/go1/bin/go", "go1");
            workspace.write_tool("cellar/go2/bin/go", "go2");
            symlink(
                workspace.path("cellar/go1/bin/go"),
                workspace.path("bin/go"),
            )
            .unwrap();
            let identities = workspace.identities(&["bin"]);
            let first = identity_of(&identities, Tool::Go).await;

            std::fs::remove_file(workspace.path("bin/go")).unwrap();
            symlink(
                workspace.path("cellar/go2/bin/go"),
                workspace.path("bin/go"),
            )
            .unwrap();
            assert_ne!(identity_of(&identities, Tool::Go).await, first);
        }

        #[tokio::test]
        async fn a_replaced_link_target_runs_again() {
            let workspace = Workspace::new();
            workspace.write_tool("cellar/go1/bin/go", "go1");
            symlink(
                workspace.path("cellar/go1/bin/go"),
                workspace.path("bin/go"),
            )
            .unwrap();
            let identities = workspace.identities(&["bin"]);
            let first = identity_of(&identities, Tool::Go).await;

            workspace.write_tool("cellar/go1/bin/go", "go2");
            assert_ne!(identity_of(&identities, Tool::Go).await, first);
        }

        #[tokio::test]
        async fn a_switched_directory_link_runs_again() {
            // Homebrew links `opt/llvm` to the installed version, and `PATH` names
            // `opt/llvm/bin`.
            let workspace = Workspace::new();
            workspace.write_tool("cellar/llvm1/bin/clang", "clang1");
            workspace.write_tool("cellar/llvm2/bin/clang", "clang2");
            std::fs::create_dir(workspace.path("opt")).unwrap();
            symlink(workspace.path("cellar/llvm1"), workspace.path("opt/llvm")).unwrap();
            let identities = workspace.identities(&["opt/llvm/bin"]);
            let first = identity_of(&identities, Tool::Clang).await;

            std::fs::remove_file(workspace.path("opt/llvm")).unwrap();
            symlink(workspace.path("cellar/llvm2"), workspace.path("opt/llvm")).unwrap();
            assert_ne!(identity_of(&identities, Tool::Clang).await, first);
        }

        #[tokio::test]
        async fn a_tool_earlier_on_path_runs_again() {
            let workspace = Workspace::new();
            workspace.write_tool("late/go", "go1");
            let identities = workspace.identities(&["bin", "late"]);
            let first = identity_of(&identities, Tool::Go).await;

            workspace.write_tool("bin/go", "go2");
            assert_ne!(identity_of(&identities, Tool::Go).await, first);
        }

        #[tokio::test]
        async fn an_installed_tool_runs() {
            let workspace = Workspace::new();
            let identities = workspace.identities(&["bin"]);
            let missing = identity_of(&identities, Tool::Go).await;
            assert_eq!(identity_of(&identities, Tool::Go).await, missing);

            workspace.write_tool("bin/go", "go1");
            assert_ne!(identity_of(&identities, Tool::Go).await, missing);
            assert_eq!(workspace.runs(), 1);
        }

        #[tokio::test]
        async fn a_go_mod_runs_go_again() {
            let workspace = Workspace::new();
            workspace.write_tool("bin/go", "go1");
            let identities = workspace.identities(&["bin"]);
            identity_of(&identities, Tool::Go).await;

            std::fs::write(workspace.path("go.mod"), "toolchain go1.99.0\n").unwrap();
            identity_of(&identities, Tool::Go).await;
            assert_eq!(workspace.runs(), 2);

            std::fs::write(workspace.path("goenv"), "GOTOOLCHAIN=local\n").unwrap();
            identity_of(&identities, Tool::Go).await;
            assert_eq!(workspace.runs(), 3);
        }

        #[tokio::test]
        async fn a_rustup_selection_runs_rustc_again() {
            let workspace = Workspace::new();
            workspace.write_tool("toolchains/a/bin/rustc", "rustc a");
            workspace.write_rustup_proxy("bin/rustc", "toolchains/a");
            let identities = workspace.identities(&["bin"]);
            identity_of(&identities, Tool::Rustc).await;
            identity_of(&identities, Tool::Rustc).await;
            assert_eq!(workspace.runs(), 1);

            std::fs::write(workspace.path("rust-toolchain.toml"), "[toolchain]\n").unwrap();
            identity_of(&identities, Tool::Rustc).await;
            assert_eq!(workspace.runs(), 2);

            std::fs::write(workspace.path("rustup/settings.toml"), "default = 'a'\n").unwrap();
            identity_of(&identities, Tool::Rustc).await;
            assert_eq!(workspace.runs(), 3);
        }

        #[tokio::test]
        async fn an_updated_toolchain_runs_rustc_again() {
            let workspace = Workspace::new();
            workspace.write_tool("toolchains/a/bin/rustc", "rustc 1");
            workspace.write_rustup_proxy("bin/rustc", "toolchains/a");
            let identities = workspace.identities(&["bin"]);
            let first = identity_of(&identities, Tool::Rustc).await;

            workspace.write_tool("toolchains/a/bin/rustc", "rustc 2");
            assert_ne!(identity_of(&identities, Tool::Rustc).await, first);
        }

        #[tokio::test]
        async fn a_failed_locate_is_not_cached() {
            let workspace = Workspace::new();
            workspace.write_script(
                "bin/rustc",
                &format!(
                    "echo run >> '{}'\n[ \"$1\" = --print ] && exit 1\necho rustc\n",
                    workspace.path("runs.log").display()
                ),
            );
            let identities = workspace.identities(&["bin"]);
            identity_of(&identities, Tool::Rustc).await;
            identity_of(&identities, Tool::Rustc).await;
            // Each command runs `--print sysroot` and `-vV`.
            assert_eq!(workspace.runs(), 4);
        }
    }
}
