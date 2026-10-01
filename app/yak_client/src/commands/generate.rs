/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

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

/// The cargo cell. Its path names no directory, because the cell exists only in memory. It
/// generates the members' targets from `cargo metadata` each time a manifest changes.
fn cargo_cell() -> init::ExternalCell {
    init::ExternalCell {
        name: "crates".to_owned(),
        path: ".crates".to_owned(),
        origin: "cargo",
        comment: "The third-party crates of the Cargo workspace, which yak generates from `cargo metadata`."
            .to_owned(),
        settings: Vec::new(),
    }
}

/// The go cell of the Go module in `dir`, relative to the project root. Its path names no
/// directory either. The module at the root
/// has the cell `gomod`, and any other the cell `gomod_<dir>`, with each character of the
/// directory that is not a letter or digit replaced by `_`.
fn go_cell(dir: &str) -> init::ExternalCell {
    let name = if dir.is_empty() {
        "gomod".to_owned()
    } else {
        let suffix: String = dir
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        format!("gomod_{suffix}")
    };
    let module = if dir.is_empty() {
        "go.mod".to_owned()
    } else {
        format!("{dir}/go.mod")
    };
    init::ExternalCell {
        path: format!(".{name}"),
        name,
        origin: "go",
        comment: format!(
            "The packages of the Go module `{module}` and its dependencies, which yak generates from `go list`."
        ),
        settings: vec![("module", module)],
    }
}

/// The build file of a directory, which loads and calls the macros of `cells` (the cell name and
/// the macro's file and name, in order).
fn build_file(calls: &[(String, &str, &str)]) -> String {
    let mut contents = String::new();
    for (cell, file, name) in calls {
        contents.push_str(&format!("load(\"@{cell}//:{file}\", \"{name}\")\n"));
    }
    contents.push('\n');
    for (_, _, name) in calls {
        contents.push_str(&format!("{name}()\n"));
    }
    contents
}

/// The lines that an existing `.yakconfig` gains for `cell`, as sections that the parser merges
/// with the earlier sections of the same name.
fn cell_config(cell: &init::ExternalCell) -> String {
    format!(
        "\n# {}\n[cells]\n  {} = {}\n\n[external_cells]\n  {} = {}\n{}",
        cell.comment,
        cell.name,
        cell.path,
        cell.name,
        cell.origin,
        cell.settings_section()
    )
}

/// Writes the build files that let yak build the Cargo workspace and the Go modules at \[PATH\].
///
/// If \[PATH\] holds a Cargo workspace, the `YAK` file at its root declares the targets of every
/// workspace member from its `Cargo.toml`, and `.yakconfig` gains the `crates` cell, which builds
/// the third-party crates in `Cargo.lock`. Each Go module below \[PATH\] gets a `YAK` file next to
/// its `go.mod`, which declares the targets of its packages, and a go cell, which builds its
/// third-party modules. The generated files do not list dependencies, so an edit to a manifest
/// or an import needs no new run.
#[derive(Debug, clap::Parser)]
#[clap(
    name = "generate",
    about = "Generate build files for a Cargo workspace and Go modules"
)]
pub struct GenerateCommand {
    /// The directory of the Cargo workspace or the Go modules, which becomes the root of the yak
    /// project.
    #[clap(default_value = ".")]
    path: PathArg,

    /// Replace a generated `YAK` file that differs from the generated one.
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

/// Adds `cells` to `.yakconfig`, and creates the project files of `yak init` when the project has
/// no `.yakconfig`.
fn configure_project(root: &AbsPath, cells: &[init::ExternalCell]) -> yak_error::Result<Written> {
    let config_path = root.join(".yakconfig");
    let Some(mut config) = fs_util::read_to_string_if_exists(&config_path)? else {
        init::set_up_yakroot(root)?;
        init::initialize_yakconfig(root, true, false, cells)?;
        let toolchains = root.join("toolchains");
        if !toolchains.exists() {
            fs_util::create_dir(&toolchains).categorize_internal()?;
            init::initialize_toolchains_yak(&toolchains)?;
        }
        return Ok(Written::Created);
    };

    let mut added = false;
    for cell in cells {
        let assigns_cell = |line: &str| {
            line.trim()
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == cell.name)
        };
        let is_origin = |line: &str| {
            line.trim()
                .split_once('=')
                .is_some_and(|(key, value)| key.trim() == cell.name && value.trim() == cell.origin)
        };
        if config.lines().any(is_origin) {
            continue;
        }
        if config.lines().any(assigns_cell) {
            return Err(yak_error!(
                ErrorTag::Input,
                "`{}` already sets `{}`, which `yak generate` configures as a {} cell",
                config_path.display(),
                cell.name,
                cell.origin
            ));
        }
        if !config.is_empty() && !config.ends_with('\n') {
            config.push('\n');
        }
        config.push_str(&cell_config(cell));
        added = true;
    }
    if !added {
        return Ok(Written::Unchanged);
    }
    fs_util::write(&config_path, config).categorize_internal()?;
    Ok(Written::Replaced)
}

/// The directories below `root` that hold a `go.mod`, relative to it, sorted. It skips the
/// directories that `go` skips in `./...` (names starting with `.` or `_`, and `testdata`),
/// `vendor`, and the output directories of yak, Cargo, and npm.
fn go_module_dirs(root: &AbsPath) -> yak_error::Result<Vec<String>> {
    let mut dirs = Vec::new();
    let mut pending = vec![String::new()];
    while let Some(dir) = pending.pop() {
        let path = if dir.is_empty() {
            root.to_owned()
        } else {
            root.join(dir.as_str())
        };
        for entry in std::fs::read_dir(path.as_path())
            .with_yak_error_context(|| format!("Error listing `{}`", path.display()))?
        {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let file_type = entry.file_type()?;
            if file_type.is_file() && name == "go.mod" {
                dirs.push(dir.clone());
            } else if file_type.is_dir()
                && !name.starts_with(['.', '_'])
                && !matches!(
                    name.as_str(),
                    "testdata" | "vendor" | "yak-out" | "target" | "node_modules"
                )
            {
                pending.push(if dir.is_empty() {
                    name
                } else {
                    format!("{dir}/{name}")
                });
            }
        }
    }
    dirs.sort();
    Ok(dirs)
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
    let cargo = root.join("Cargo.toml").exists();
    let go_modules = go_module_dirs(root)?;
    if !cargo && go_modules.is_empty() {
        return Err(yak_error!(
            ErrorTag::Input,
            "`{}` has no `Cargo.toml` and no Go module. `yak generate` runs in the root of a \
             Cargo workspace or in a directory that holds Go modules.",
            root.display()
        ));
    }

    let mut cells = Vec::new();
    // The macros that the build file of each directory calls, by directory.
    let mut calls: BTreeMap<String, Vec<(String, &str, &str)>> = BTreeMap::new();
    let mut descriptions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if cargo {
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

        // All members' targets are in the package at the root of the workspace, which a build
        // file in a member's directory would cut the member out of.
        let members = layout.member_dirs()?;
        let mut splitting: Vec<String> = members
            .iter()
            .filter(|dir| !dir.is_empty() && root.join(dir.as_str()).join("YAK").exists())
            .map(|dir| format!("`{dir}/YAK`"))
            .collect();
        splitting.extend(
            go_modules
                .iter()
                .filter(|dir| !dir.is_empty() && members.contains(dir))
                .map(|dir| format!("the Go module's `{dir}/YAK`")),
        );
        if !splitting.is_empty() {
            return Err(yak_error!(
                ErrorTag::Input,
                "{} would make workspace members packages of their own, which the build file at \
                 the root of the workspace cannot reach. Remove them and run `yak generate` again.",
                splitting.join(", ")
            ));
        }
        let cell = cargo_cell();
        calls.entry(String::new()).or_default().push((
            cell.name.clone(),
            "workspace.bzl",
            "cargo_workspace",
        ));
        descriptions.entry(String::new()).or_default().push(format!(
            "the targets of {} workspace members",
            members.len()
        ));
        cells.push(cell);
    }
    for dir in &go_modules {
        let cell = go_cell(dir);
        if let Some(other) = cells.iter().find(|c| c.name == cell.name) {
            return Err(yak_error!(
                ErrorTag::Input,
                "The Go modules in `{dir}` and of `{}` would both have the cell `{}`",
                other
                    .settings
                    .first()
                    .map_or("", |(_, module)| module.as_str()),
                cell.name
            ));
        }
        calls
            .entry(dir.clone())
            .or_default()
            .push((cell.name.clone(), "module.bzl", "go_module"));
        descriptions
            .entry(dir.clone())
            .or_default()
            .push("the targets of the Go module's packages".to_owned());
        cells.push(cell);
    }

    let config = configure_project(root, &cells)?;
    ignore_yak_out(root)?;
    match config {
        Written::Created => console.print_success(&format!(
            "Created `.yakconfig` with the cells {}",
            cell_names(&cells)
        ))?,
        Written::Replaced => console.print_success(&format!(
            "Configured {} in `.yakconfig`",
            cell_names(&cells)
        ))?,
        Written::Unchanged | Written::Kept => {}
    }
    for (dir, calls) in &calls {
        let relative = if dir.is_empty() {
            "YAK".to_owned()
        } else {
            format!("{dir}/YAK")
        };
        let path = root.join(relative.as_str());
        match write_if_changed(&path, &build_file(calls), cmd.force)? {
            Written::Created | Written::Replaced => console.print_success(&format!(
                "Wrote `{relative}`, which declares {}",
                descriptions[dir].join(" and ")
            ))?,
            Written::Unchanged => console.print_success(&format!("`{relative}` is up to date"))?,
            Written::Kept => console.print_warning(&format!(
                "Kept `{}`, which differs from the generated build file. Pass `--force` to \
                 replace it, or add the generated lines to it:\n{}",
                path.display(),
                build_file(calls)
            ))?,
        }
    }
    Ok(())
}

fn cell_names(cells: &[init::ExternalCell]) -> String {
    cells
        .iter()
        .map(|c| format!("`{}`", c.name))
        .collect::<Vec<_>>()
        .join(", ")
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
        let cells = [cargo_cell(), go_cell("services/api")];
        assert_eq!(configure_project(root, &cells).unwrap(), Written::Replaced);
        assert_eq!(configure_project(root, &cells).unwrap(), Written::Unchanged);
        let config = fs_util::read_to_string(root.join(".yakconfig")).unwrap();
        assert_eq!(
            config,
            format!(
                "[cells]\n  root = .\n{}{}",
                cell_config(&cells[0]),
                cell_config(&cells[1])
            )
        );
        assert!(config.ends_with(
            "[external_cells]\n  gomod_services_api = go\n\n[external_cell_gomod_services_api]\n  module = services/api/go.mod\n"
        ));
    }

    #[test]
    fn test_configure_project_rejects_another_crates_cell() {
        let dir = tempfile::tempdir().unwrap();
        let root = AbsPath::new(dir.path()).unwrap();
        fs_util::write(root.join(".yakconfig"), "[cells]\n  crates = third-party\n").unwrap();
        assert!(configure_project(root, &[cargo_cell()]).is_err());
    }

    #[test]
    fn test_build_file() {
        assert_eq!(
            build_file(&[
                ("crates".to_owned(), "workspace.bzl", "cargo_workspace"),
                ("gomod".to_owned(), "module.bzl", "go_module"),
            ]),
            "load(\"@crates//:workspace.bzl\", \"cargo_workspace\")\nload(\"@gomod//:module.bzl\", \"go_module\")\n\ncargo_workspace()\ngo_module()\n"
        );
    }

    #[test]
    fn test_go_module_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = AbsPath::new(dir.path()).unwrap();
        for module in [
            "",
            "services/api",
            "services/api/testdata/fixture",
            ".hidden/m",
            "yak-out/m",
            "tools/_old",
        ] {
            let path = if module.is_empty() {
                root.to_owned()
            } else {
                root.join(module)
            };
            fs_util::create_dir_all(&path).unwrap();
            fs_util::write(path.join("go.mod"), "module m\n").unwrap();
        }
        assert_eq!(go_module_dirs(root).unwrap(), ["", "services/api"]);
        assert_eq!(go_cell("services/api").name, "gomod_services_api");
        assert_eq!(go_cell("").name, "gomod");
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
