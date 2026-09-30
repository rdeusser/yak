/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use yak_client_ctx::client_ctx::ClientCommandContext;
use yak_client_ctx::common::YakArgMatches;
use yak_client_ctx::common::ui::CommonConsoleOptions;
use yak_client_ctx::exit_result::ExitResult;
use yak_client_ctx::final_console::FinalConsole;
use yak_client_ctx::path_arg::PathArg;
use yak_common::argv::Argv;
use yak_common::argv::SanitizedArgv;
use yak_error::ErrorTag;
use yak_error::YakErrorContext;
use yak_error::yak_error;
use yak_external_cells_cargo::metadata::WorkspaceLayout;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_path::AbsPath;
use yak_util::process::background_command;

use crate::commands::init;

/// The build file at the root of the workspace. The cargo cell generates the members' targets
/// from `cargo metadata` each time a manifest changes.
const WORKSPACE_BUILD_FILE: &str = "load(\"@crates//:workspace.bzl\", \"cargo_workspace\")\n\
                                    \n\
                                    cargo_workspace()\n";

/// The comment above the cargo cell's configuration.
const CARGO_CELL_COMMENT: &str =
    "The third-party crates of the Cargo workspace, which yak generates from `cargo metadata`.";

/// The cargo cell. Its path names no directory, because the cell exists only in memory.
const CARGO_CELL: init::ExternalCell = init::ExternalCell {
    name: "crates",
    path: ".crates",
    origin: "cargo",
    comment: CARGO_CELL_COMMENT,
};

/// The lines that an existing `.yakconfig` gains, as sections that the parser merges with the
/// earlier sections of the same name.
fn cargo_cell_config() -> String {
    format!(
        "\n# {}\n[cells]\n  {} = {}\n\n[external_cells]\n  {} = {}\n",
        CARGO_CELL.comment, CARGO_CELL.name, CARGO_CELL.path, CARGO_CELL.name, CARGO_CELL.origin
    )
}

/// Writes the build files that let yak build the Cargo workspace at \[PATH\].
///
/// The `YAK` file at the root of the workspace declares the targets of every workspace member from
/// its `Cargo.toml`, and `.yakconfig` gains the `crates` cell, which builds the third-party crates
/// in `Cargo.lock`. The generated files do not list dependencies, so an edit to a manifest needs
/// no new run.
#[derive(Debug, clap::Parser)]
#[clap(
    name = "generate",
    about = "Generate build files for a Cargo workspace"
)]
pub struct GenerateCommand {
    /// The directory of the Cargo workspace, which becomes the root of the yak project.
    #[clap(default_value = ".")]
    path: PathArg,

    /// Replace a `YAK` file at the root of the workspace that differs from the generated one.
    #[clap(long)]
    force: bool,

    #[clap(flatten)]
    console_opts: CommonConsoleOptions,
}

impl GenerateCommand {
    pub fn exec(self, _matches: YakArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let console = self.console_opts.final_console();
        match exec_impl(self, ctx, &console) {
            Ok(()) => ExitResult::success(),
            Err(e) => e.into(),
        }
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

/// What `write_if_changed` did with a file.
#[derive(Debug, PartialEq, Eq)]
enum Written {
    Created,
    Replaced,
    Unchanged,
    /// The file has other contents, which the command left in place.
    Kept,
}

fn write_if_changed(path: &AbsPath, contents: &str, replace: bool) -> yak_error::Result<Written> {
    let existing = match fs_util::read_to_string_if_exists(path)? {
        None => {
            fs_util::write(path, contents).categorize_internal()?;
            return Ok(Written::Created);
        }
        Some(existing) => existing,
    };
    if existing == contents {
        Ok(Written::Unchanged)
    } else if replace {
        fs_util::write(path, contents).categorize_internal()?;
        Ok(Written::Replaced)
    } else {
        Ok(Written::Kept)
    }
}

fn workspace_layout(root: &AbsPath) -> yak_error::Result<WorkspaceLayout> {
    let output = background_command("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .yak_error_context("Could not run `cargo metadata`, which `yak generate` needs")?;
    if !output.status.success() {
        return Err(yak_error!(
            ErrorTag::Input,
            "`cargo metadata --no-deps` failed with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    WorkspaceLayout::parse(&String::from_utf8(output.stdout)?)
}

/// Adds the cargo cell to `.yakconfig`, and creates the project files of `yak init` when the
/// project has no `.yakconfig`.
fn configure_project(root: &AbsPath) -> yak_error::Result<Written> {
    let config_path = root.join(".yakconfig");
    let Some(config) = fs_util::read_to_string_if_exists(&config_path)? else {
        init::set_up_yakroot(root)?;
        init::initialize_yakconfig(root, true, false, &[CARGO_CELL])?;
        let toolchains = root.join("toolchains");
        if !toolchains.exists() {
            fs_util::create_dir(&toolchains).categorize_internal()?;
            init::initialize_toolchains_yak(&toolchains)?;
        }
        return Ok(Written::Created);
    };

    let assigns_cell = |line: &str| {
        line.trim()
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == CARGO_CELL.name)
    };
    let is_cargo_origin = |line: &str| {
        line.trim().split_once('=').is_some_and(|(key, value)| {
            key.trim() == CARGO_CELL.name && value.trim() == CARGO_CELL.origin
        })
    };
    if config.lines().any(is_cargo_origin) {
        return Ok(Written::Unchanged);
    }
    if config.lines().any(assigns_cell) {
        return Err(yak_error!(
            ErrorTag::Input,
            "`{}` already sets `{}`, which `yak generate` configures as the cargo cell",
            config_path.display(),
            CARGO_CELL.name
        ));
    }
    let mut config = config;
    if !config.is_empty() && !config.ends_with('\n') {
        config.push('\n');
    }
    fs_util::write(&config_path, config + &cargo_cell_config()).categorize_internal()?;
    Ok(Written::Replaced)
}

/// Adds `/yak-out` to an existing `.gitignore` that does not ignore it.
fn ignore_yak_out(root: &AbsPath) -> yak_error::Result<()> {
    let path = root.join(".gitignore");
    let Some(mut gitignore) = fs_util::read_to_string_if_exists(&path)? else {
        return Ok(());
    };
    let ignored = gitignore
        .lines()
        .any(|l| matches!(l.trim(), "yak-out" | "/yak-out" | "yak-out/" | "/yak-out/"));
    if !ignored {
        if !gitignore.is_empty() && !gitignore.ends_with('\n') {
            gitignore.push('\n');
        }
        gitignore.push_str("/yak-out\n");
        fs_util::write(&path, gitignore).categorize_internal()?;
    }
    Ok(())
}

fn exec_impl(
    cmd: GenerateCommand,
    ctx: ClientCommandContext<'_>,
    console: &FinalConsole,
) -> yak_error::Result<()> {
    let root = fs_util::canonicalize(cmd.path.resolve(&ctx.working_dir)).categorize_internal()?;
    let root = root.as_abs_path();
    if !root.join("Cargo.toml").exists() {
        return Err(yak_error!(
            ErrorTag::Input,
            "`{}` has no `Cargo.toml`. `yak generate` runs in the root of a Cargo workspace.",
            root.display()
        ));
    }

    let layout = workspace_layout(root)?;
    let workspace_root =
        fs_util::canonicalize(AbsPath::new(&layout.workspace_root)?).categorize_internal()?;
    if workspace_root.as_abs_path() != root {
        return Err(yak_error!(
            ErrorTag::Input,
            "`{}` is a member of the Cargo workspace at `{}`. Run `yak generate` there.",
            root.display(),
            workspace_root.display()
        ));
    }

    // All members' targets are in the package at the root of the workspace, which a build file
    // in a member's directory would cut the member out of.
    let members = layout.member_dirs()?;
    let splitting: Vec<String> = members
        .iter()
        .filter(|dir| !dir.is_empty() && root.join(dir.as_str()).join("YAK").exists())
        .map(|dir| format!("`{dir}/YAK`"))
        .collect();
    if !splitting.is_empty() {
        return Err(yak_error!(
            ErrorTag::Input,
            "{} would make workspace members packages of their own, which the build file at the \
             root of the workspace cannot reach. Remove them and run `yak generate` again.",
            splitting.join(", ")
        ));
    }

    let config = configure_project(root)?;
    ignore_yak_out(root)?;
    let build_file = root.join("YAK");
    let written = write_if_changed(&build_file, WORKSPACE_BUILD_FILE, cmd.force)?;

    match config {
        Written::Created => console.print_success("Created `.yakconfig` with the `crates` cell")?,
        Written::Replaced => console.print_success("Added the `crates` cell to `.yakconfig`")?,
        Written::Unchanged | Written::Kept => {}
    }
    match written {
        Written::Created | Written::Replaced => console.print_success(&format!(
            "Wrote `YAK`, which declares the targets of {} workspace members",
            members.len()
        ))?,
        Written::Unchanged => console.print_success("`YAK` is up to date")?,
        Written::Kept => console.print_warning(&format!(
            "Kept `{}`, which differs from the generated build file. Pass `--force` to replace \
             it, or add its two lines to it.",
            build_file.display()
        ))?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_if_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = AbsPath::new(dir.path()).unwrap().join("YAK");
        assert_eq!(
            write_if_changed(&path, "a", false).unwrap(),
            Written::Created
        );
        assert_eq!(
            write_if_changed(&path, "a", false).unwrap(),
            Written::Unchanged
        );
        assert_eq!(write_if_changed(&path, "b", false).unwrap(), Written::Kept);
        assert_eq!(fs_util::read_to_string(&path).unwrap(), "a");
        assert_eq!(
            write_if_changed(&path, "b", true).unwrap(),
            Written::Replaced
        );
        assert_eq!(fs_util::read_to_string(&path).unwrap(), "b");
    }

    #[test]
    fn test_configure_project_appends_the_cell_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = AbsPath::new(dir.path()).unwrap();
        fs_util::write(root.join(".yakconfig"), "[cells]\n  root = .").unwrap();
        assert_eq!(configure_project(root).unwrap(), Written::Replaced);
        assert_eq!(configure_project(root).unwrap(), Written::Unchanged);
        let config = fs_util::read_to_string(root.join(".yakconfig")).unwrap();
        assert_eq!(
            config,
            format!("[cells]\n  root = .\n{}", cargo_cell_config())
        );
    }

    #[test]
    fn test_configure_project_rejects_another_crates_cell() {
        let dir = tempfile::tempdir().unwrap();
        let root = AbsPath::new(dir.path()).unwrap();
        fs_util::write(root.join(".yakconfig"), "[cells]\n  crates = third-party\n").unwrap();
        assert!(configure_project(root).is_err());
    }

    #[test]
    fn test_ignore_yak_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = AbsPath::new(dir.path()).unwrap();
        fs_util::write(root.join(".gitignore"), "/target").unwrap();
        ignore_yak_out(root).unwrap();
        ignore_yak_out(root).unwrap();
        assert_eq!(
            fs_util::read_to_string(root.join(".gitignore")).unwrap(),
            "/target\n/yak-out\n"
        );
    }
}
