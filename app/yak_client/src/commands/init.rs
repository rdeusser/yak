/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::ErrorKind;
use std::io::Write;

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
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_path::AbsPath;
use yak_util::process::background_command;

/// Initializes a yak project at the provided path.
#[derive(Debug, clap::Parser)]
#[clap(name = "init", about = "Initialize a yak project")]
pub struct InitCommand {
    /// The path to initialize the project in. The folder does not need to exist.
    #[clap(default_value = ".")]
    path: PathArg,

    /// Don't include the standard prelude or generate toolchain definitions.
    #[clap(long)]
    no_prelude: bool,

    /// Initialize the project even if the git repo at \[PATH\] has uncommitted changes.
    #[clap(long)]
    allow_dirty: bool,

    /// Also initialize a git repository at the given path, and set up an appropriate `.gitignore`
    /// file.
    #[clap(long)]
    git: bool,

    #[clap(flatten)]
    console_opts: CommonConsoleOptions,
}

impl InitCommand {
    pub fn exec(self, _matches: YakArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let console = self.console_opts.final_console();

        match exec_impl(self, ctx, &console) {
            Ok(_) => ExitResult::success(),
            Err(e) => {
                // include the backtrace with the error output
                // (same behaviour as returning the Error from main)
                yak_error!(ErrorTag::Tier0, "{:?}", e).into()
            }
        }
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

fn exec_impl(
    cmd: InitCommand,
    ctx: ClientCommandContext<'_>,
    console: &FinalConsole,
) -> yak_error::Result<()> {
    let path = cmd.path.resolve(&ctx.working_dir);
    fs_util::create_dir_all(&path)?;
    let absolute = fs_util::canonicalize(&path).categorize_internal()?;
    let git = cmd.git;

    if absolute.is_file() {
        return Err(yak_error!(
            yak_error::ErrorTag::Input,
            "Target path {} cannot be an existing file",
            absolute.display()
        ));
    }

    if git {
        let status = match background_command("git")
            .args(["status", "--porcelain"])
            .current_dir(&absolute)
            .output()
        {
            Err(e) if e.kind().eq(&ErrorKind::NotFound) => {
                console.print_error(
                    "Warning: no git found on path, can't check for dirty repo. Proceeding anyway.",
                )?;
                None
            }
            r => Some(r.yak_error_context("Couldn't detect dirty status of folder.")?),
        };

        let changes = status.filter(|o| o.status.success()).map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .lines()
                .any(|l| !l.starts_with("??"))
        });

        if let (Some(true), false) = (changes, cmd.allow_dirty) {
            return Err(yak_error!(
                yak_error::ErrorTag::Input,
                "Refusing to initialize in a dirty repo. Stash your changes or use `--allow-dirty` to override."
            ));
        }
    }

    set_up_project(&absolute, git, !cmd.no_prelude)
}

/// ExternalCell is a cell of a new `.yakconfig` beyond those of `yak init`.
pub(crate) struct ExternalCell {
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
    pub(crate) origin: &'static str,
    /// The comment above the cell's `[external_cells]` entry.
    pub(crate) comment: &'static str,
}

/// Writes `.yakconfig`. The `external_cells` need the prelude.
pub(crate) fn initialize_yakconfig(
    repo_root: &AbsPath,
    prelude: bool,
    git: bool,
    external_cells: &[ExternalCell],
) -> yak_error::Result<()> {
    let mut yakconfig = std::fs::File::create(repo_root.join(".yakconfig"))?;
    writeln!(yakconfig, "[cells]")?;
    writeln!(yakconfig, "  root = .")?;

    // Add additional configs that depend on prelude / no-prelude mode
    if prelude {
        writeln!(yakconfig, "  prelude = prelude")?;
        writeln!(yakconfig, "  toolchains = toolchains")?;
        for cell in external_cells {
            writeln!(yakconfig, "  {} = {}", cell.name, cell.path)?;
        }
        writeln!(yakconfig)?;
        writeln!(yakconfig, "[cell_aliases]")?;
        writeln!(yakconfig, "  config = prelude")?;
        writeln!(yakconfig)?;
        writeln!(
            yakconfig,
            "# Uses a copy of the prelude bundled with the yak binary. You can alternatively delete this"
        )?;
        writeln!(
            yakconfig,
            "# section and vendor a copy of the prelude to the `prelude` directory of your project."
        )?;
        writeln!(yakconfig, "[external_cells]")?;
        writeln!(yakconfig, "  prelude = bundled")?;
        for cell in external_cells {
            writeln!(yakconfig, "# {}", cell.comment)?;
            writeln!(yakconfig, "  {} = {}", cell.name, cell.origin)?;
        }
        writeln!(yakconfig)?;
        writeln!(yakconfig, "[parser]")?;
        write!(
            yakconfig,
            "  target_platform_detector_spec = target:root//...->prelude//platforms:default \\
    target:prelude//...->prelude//platforms:default \\
    target:toolchains//...->prelude//platforms:default"
        )?;
        for cell in external_cells {
            write!(
                yakconfig,
                " \\\n    target:{}//...->prelude//platforms:default",
                cell.name
            )?;
        }
        writeln!(yakconfig)?;
        writeln!(yakconfig)?;
        writeln!(yakconfig, "[build]")?;
        writeln!(
            yakconfig,
            "  execution_platforms = prelude//platforms:default"
        )?;
    }

    if git {
        writeln!(yakconfig)?;
        writeln!(yakconfig, "[project]")?;
        writeln!(yakconfig, "  ignore = .git")?;
    }
    Ok(())
}

pub(crate) fn initialize_toolchains_yak(repo_root: &AbsPath) -> yak_error::Result<()> {
    std::fs::write(
        repo_root.join("YAK"),
        r#"
load("@prelude//toolchains:system.bzl", "system_toolchains")

# A toolchain for each language, using the compilers and tools on `PATH`.
system_toolchains()
"#
        .trim(),
    )?;
    Ok(())
}

fn initialize_root_yak(repo_root: &AbsPath, prelude: bool) -> yak_error::Result<()> {
    let mut yak = std::fs::File::create(repo_root.join("YAK"))?;

    if prelude {
        writeln!(
            yak,
            "# A list of available rules and their signatures can be found here: https://rdeusser.github.io/yak/docs/prelude/globals/"
        )?;
        writeln!(yak)?;
        writeln!(yak, "genrule(")?;
        writeln!(yak, "    name = \"hello_world\",")?;
        writeln!(yak, "    out = \"out.txt\",")?;
        writeln!(yak, "    cmd = \"echo BUILT BY YAK> $OUT\",")?;
        writeln!(yak, ")")?;
    }
    // TODO: Add a doc pointers for rules
    Ok(())
}

fn set_up_gitignore(repo_root: &AbsPath) -> yak_error::Result<()> {
    let gitignore = repo_root.join(".gitignore");
    // If .gitignore is empty or doesn't exist, add in yak-out
    if !gitignore.exists() || fs_util::metadata(&gitignore).categorize_internal()?.len() == 0 {
        fs_util::write(gitignore, "/yak-out\n").categorize_internal()?;
    }
    Ok(())
}

pub(crate) fn set_up_yakroot(repo_root: &AbsPath) -> yak_error::Result<()> {
    fs_util::write(repo_root.join(".yakroot"), "").categorize_internal()?;
    Ok(())
}

fn set_up_project(repo_root: &AbsPath, git: bool, prelude: bool) -> yak_error::Result<()> {
    set_up_yakroot(repo_root)?;

    if git {
        if !background_command("git")
            .arg("init")
            .current_dir(repo_root)
            .status()?
            .success()
        {
            return Err(yak_error!(
                yak_error::ErrorTag::Tier0,
                "Failure when running `git init`."
            ));
        };
        set_up_gitignore(repo_root)?;
    }

    // If the project already contains a .yakconfig, leave it alone
    if repo_root.join(".yakconfig").exists() {
        yak_client_ctx::println!(
            ".yakconfig already exists, not overwriting and not generating toolchains"
        )?;
        return Ok(());
    }

    initialize_yakconfig(repo_root, prelude, git, &[])?;
    if prelude {
        let toolchains = repo_root.join("toolchains");
        if !toolchains.exists() {
            fs_util::create_dir(&toolchains).categorize_internal()?;
            initialize_toolchains_yak(&toolchains)?;
        }
    }
    if !repo_root.join("YAK").exists() {
        initialize_root_yak(repo_root, prelude)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use yak_fs::fs_util::uncategorized as fs_util;
    use yak_fs::paths::abs_path::AbsPath;

    use crate::commands::init::initialize_root_yak;
    use crate::commands::init::initialize_yakconfig;
    use crate::commands::init::set_up_gitignore;
    use crate::commands::init::set_up_project;

    #[test]
    fn test_set_up_project_with_prelude_no_git() -> yak_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        // no git, with prelude
        set_up_project(tempdir_path, false, true)?;
        assert!(tempdir_path.join(".yakconfig").exists());
        assert!(tempdir_path.join("toolchains").exists());
        assert!(tempdir_path.join("toolchains/YAK").exists());
        assert!(tempdir_path.join("YAK").exists());
        Ok(())
    }

    #[test]
    fn test_default_gitignore() -> yak_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        // .gitignore does not exist yet
        set_up_gitignore(tempdir_path)?;
        let gitignore_path = tempdir_path.join(".gitignore");
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "/yak-out\n";
        assert_eq!(actual, expected);

        // If an empty .yakconfig exists (this is the case we would hit after running `git init`), add `yak-out`
        fs_util::write(&gitignore_path, "")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        assert_eq!(actual, expected);

        // If a non-empty.yakconfig exists, don't touch it
        fs_util::write(&gitignore_path, "foo\nbar\n")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "foo\nbar\n";
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn test_yakconfig_generation_with_prelude() -> yak_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let yakconfig_path = tempdir_path.join(".yakconfig");
        initialize_yakconfig(tempdir_path, true, true, &[])?;
        let actual_yakconfig = fs_util::read_to_string(yakconfig_path)?;
        let expected_yakconfig = "[cells]
  root = .
  prelude = prelude
  toolchains = toolchains

[cell_aliases]
  config = prelude

# Uses a copy of the prelude bundled with the yak binary. You can alternatively delete this
# section and vendor a copy of the prelude to the `prelude` directory of your project.
[external_cells]
  prelude = bundled

[parser]
  target_platform_detector_spec = target:root//...->prelude//platforms:default \\
    target:prelude//...->prelude//platforms:default \\
    target:toolchains//...->prelude//platforms:default

[build]
  execution_platforms = prelude//platforms:default

[project]
  ignore = .git
";
        assert_eq!(actual_yakconfig, expected_yakconfig);
        Ok(())
    }

    #[test]
    fn test_yakconfig_generation_without_prelude() -> yak_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let yakconfig_path = tempdir_path.join(".yakconfig");
        initialize_yakconfig(tempdir_path, false, false, &[])?;
        let actual_yakconfig = fs_util::read_to_string(yakconfig_path)?;
        let expected_yakconfig = "[cells]
  root = .
";
        assert_eq!(actual_yakconfig, expected_yakconfig);

        Ok(())
    }

    #[test]
    fn test_yakfile_generation_with_prelude() -> yak_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let yak_path = tempdir_path.join("YAK");
        initialize_root_yak(tempdir_path, true)?;
        let actual_yak = fs_util::read_to_string(yak_path)?;
        let expected_yak = "# A list of available rules and their signatures can be found here: https://rdeusser.github.io/yak/docs/prelude/globals/

genrule(
    name = \"hello_world\",
    out = \"out.txt\",
    cmd = \"echo BUILT BY YAK> $OUT\",
)
";
        assert_eq!(actual_yak, expected_yak);
        Ok(())
    }
}
