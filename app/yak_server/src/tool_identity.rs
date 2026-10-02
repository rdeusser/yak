/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The identities of the tools that the system toolchains run from `PATH`, which the daemon
//! computes at the start of each command and offers as `tool_identity.<tool>` in the
//! configuration. A toolchain puts the identity of its tool into the keys of its actions, so an
//! action runs again when its compiler changes, and a remote cache keeps the results of different
//! compilers apart.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use futures::future::join_all;
use yak_common::legacy_configs::args::ComputedConfigValue;

/// TOOLS names each tool that `system_toolchains()` runs from `PATH`, with the command that
/// prints its version.
const TOOLS: &[(&str, &[&str])] = &[
    ("rustc", &["rustc", "-vV"]),
    ("go", &["go", "version"]),
    ("clang", &["clang", "--version"]),
];

/// TIMEOUT limits the run of one version command.
const TIMEOUT: Duration = Duration::from_secs(10);

/// tool_identities runs the version command of each tool in `project_root`, where `rustup` reads
/// `rust-toolchain.toml`, and returns `tool_identity.<tool>` for each tool. A `-c` of the command
/// line overrides them.
pub(crate) async fn tool_identities(project_root: &Path) -> Vec<ComputedConfigValue> {
    join_all(TOOLS.iter().map(|(tool, argv)| async move {
        ComputedConfigValue {
            section: "tool_identity".to_owned(),
            key: (*tool).to_owned(),
            value: identity(&run(argv, project_root).await),
        }
    }))
    .await
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

async fn run(argv: &[&str], cwd: &Path) -> VersionOutput {
    let output = tokio::process::Command::new(argv[0])
        .args(&argv[1..])
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

    #[tokio::test]
    async fn a_missing_tool_has_an_identity() {
        let dir = std::env::temp_dir();
        let output = run(&["yak-no-such-tool-for-identity"], &dir).await;
        assert!(matches!(
            output,
            VersionOutput::NotStarted(std::io::ErrorKind::NotFound)
        ));
    }
}
