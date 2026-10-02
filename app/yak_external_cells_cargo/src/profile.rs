/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `dev` profile of a Cargo workspace, which `cargo build` compiles with. It comes from the
//! `[profile.dev]` tables of the workspace's `Cargo.toml` and of Cargo's configuration files.

use std::collections::BTreeMap;

use yak_external_cells_starlark::Value;

use crate::rustflags::ConfigFile;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum ProfileError {
    #[error("`{path}` is not valid TOML: {error}")]
    InvalidToml { path: String, error: String },
    #[error("`{key}` in `{path}` is `{value}`, which is not {expected}")]
    InvalidValue {
        path: String,
        key: String,
        value: String,
        expected: &'static str,
    },
}

/// OptLevel is a value of `opt-level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptLevel {
    O0,
    O1,
    O2,
    O3,
    Size,
    MinSize,
}

impl OptLevel {
    fn parse(value: &toml::Value) -> Option<OptLevel> {
        let level = match value {
            toml::Value::Integer(level) => level.to_string(),
            toml::Value::String(level) => level.clone(),
            _ => return None,
        };
        match level.as_str() {
            "0" => Some(OptLevel::O0),
            "1" => Some(OptLevel::O1),
            "2" => Some(OptLevel::O2),
            "3" => Some(OptLevel::O3),
            "s" => Some(OptLevel::Size),
            "z" => Some(OptLevel::MinSize),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            OptLevel::O0 => "0",
            OptLevel::O1 => "1",
            OptLevel::O2 => "2",
            OptLevel::O3 => "3",
            OptLevel::Size => "s",
            OptLevel::MinSize => "z",
        }
    }
}

/// DebugInfo is a value of `debug`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugInfo {
    None,
    LineDirectivesOnly,
    LineTablesOnly,
    Limited,
    Full,
}

impl DebugInfo {
    fn parse(value: &toml::Value) -> Option<DebugInfo> {
        match value {
            toml::Value::Boolean(true) | toml::Value::Integer(2) => Some(DebugInfo::Full),
            toml::Value::Boolean(false) | toml::Value::Integer(0) => Some(DebugInfo::None),
            toml::Value::Integer(1) => Some(DebugInfo::Limited),
            toml::Value::String(s) => match s.as_str() {
                "none" => Some(DebugInfo::None),
                "line-directives-only" => Some(DebugInfo::LineDirectivesOnly),
                "line-tables-only" => Some(DebugInfo::LineTablesOnly),
                "limited" => Some(DebugInfo::Limited),
                "full" => Some(DebugInfo::Full),
                _ => None,
            },
            _ => None,
        }
    }

    /// The value of `-Cdebuginfo`.
    fn as_str(self) -> &'static str {
        match self {
            DebugInfo::None => "0",
            DebugInfo::LineDirectivesOnly => "line-directives-only",
            DebugInfo::LineTablesOnly => "line-tables-only",
            DebugInfo::Limited => "1",
            DebugInfo::Full => "2",
        }
    }
}

/// Panic is a value of `panic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panic {
    Unwind,
    Abort,
}

/// Overrides holds the settings that one profile table sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Overrides {
    opt_level: Option<OptLevel>,
    debuginfo: Option<DebugInfo>,
    debug_assertions: Option<bool>,
    overflow_checks: Option<bool>,
    codegen_units: Option<u32>,
}

impl Overrides {
    /// Parses the settings of `table`, whose key is `key` in the file `path`.
    fn parse(table: &toml::Table, path: &str, key: &str) -> yak_error::Result<Overrides> {
        Ok(Overrides {
            opt_level: setting(
                table,
                path,
                key,
                "opt-level",
                "0 to 3, \"s\", or \"z\"",
                OptLevel::parse,
            )?,
            debuginfo: setting(
                table,
                path,
                key,
                "debug",
                "a boolean, 0 to 2, or a level such as \"line-tables-only\"",
                DebugInfo::parse,
            )?,
            debug_assertions: setting(table, path, key, "debug-assertions", "a boolean", |v| {
                v.as_bool()
            })?,
            overflow_checks: setting(table, path, key, "overflow-checks", "a boolean", |v| {
                v.as_bool()
            })?,
            codegen_units: setting(
                table,
                path,
                key,
                "codegen-units",
                "a positive integer",
                |v| {
                    v.as_integer()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n > 0)
                },
            )?,
        })
    }

    /// The settings of `self`, with those that `higher` sets replacing them.
    fn merge(self, higher: Overrides) -> Overrides {
        Overrides {
            opt_level: higher.opt_level.or(self.opt_level),
            debuginfo: higher.debuginfo.or(self.debuginfo),
            debug_assertions: higher.debug_assertions.or(self.debug_assertions),
            overflow_checks: higher.overflow_checks.or(self.overflow_checks),
            codegen_units: higher.codegen_units.or(self.codegen_units),
        }
    }

    fn apply(&self, settings: &mut Settings) {
        if let Some(opt_level) = self.opt_level {
            settings.opt_level = opt_level;
        }
        if let Some(debuginfo) = self.debuginfo {
            settings.debuginfo = debuginfo;
        }
        if let Some(debug_assertions) = self.debug_assertions {
            settings.debug_assertions = debug_assertions;
        }
        if let Some(overflow_checks) = self.overflow_checks {
            settings.overflow_checks = overflow_checks;
        }
        if let Some(codegen_units) = self.codegen_units {
            settings.codegen_units = Some(codegen_units);
        }
    }
}

/// The value of `name` in `table`, or `None` when the table does not set it.
fn setting<T>(
    table: &toml::Table,
    path: &str,
    key: &str,
    name: &str,
    expected: &'static str,
    parse: impl Fn(&toml::Value) -> Option<T>,
) -> yak_error::Result<Option<T>> {
    let Some(value) = table.get(name) else {
        return Ok(None);
    };
    match parse(value) {
        Some(parsed) => Ok(Some(parsed)),
        None => Err(ProfileError::InvalidValue {
            path: path.to_owned(),
            key: format!("{key}.{name}"),
            value: value.to_string(),
            expected,
        }
        .into()),
    }
}

/// The constraint value of the configurations whose crates compile with `-Cpanic=abort`.
pub const PANIC_ABORT: &str = "prelude//rust/panic:panic[abort]";

/// The incoming transition that compiles a binary and its dependencies with `-Cpanic=abort`.
pub const PANIC_ABORT_TRANSITION: &str = "prelude//rust/panic:panic_transition[abort]";

/// The Starlark expression of the flags that apply the panic strategy of the configuration.
pub fn panic_flags_select() -> String {
    format!("select({{\"{PANIC_ABORT}\": [\"-Cpanic=abort\"], \"DEFAULT\": []}})")
}

/// Profile is the `dev` profile of a workspace: its `[profile.dev]` table, with the
/// `[profile.dev.build-override]` and `[profile.dev.package.<spec>]` tables in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    base: Overrides,
    panic: Option<Panic>,
    build_override: Overrides,
    /// By the package spec of the table, such as `foo`, `foo@1.2.3`, or `*`.
    packages: BTreeMap<String, Overrides>,
}

impl Profile {
    /// Parses the `[profile.dev]` table of the workspace's manifest and of `config_files`, which
    /// are ordered from the lowest precedence to the highest. The configuration files take
    /// precedence over the manifest, as with Cargo. A setting that this module does not know,
    /// such as `lto`, has no effect.
    pub fn parse(manifest: &ConfigFile, config_files: &[ConfigFile]) -> yak_error::Result<Profile> {
        let mut profile = Profile::default();
        for file in std::iter::once(manifest).chain(config_files) {
            let path = file.path.as_str();
            let table: toml::Table =
                toml::from_str(&file.contents).map_err(|e| ProfileError::InvalidToml {
                    path: path.to_owned(),
                    error: e.to_string(),
                })?;
            let Some(dev) = table
                .get("profile")
                .and_then(|p| p.as_table())
                .and_then(|p| p.get("dev"))
                .and_then(|d| d.as_table())
            else {
                continue;
            };
            profile.merge_table(dev, path)?;
        }
        Ok(profile)
    }

    fn merge_table(&mut self, dev: &toml::Table, path: &str) -> yak_error::Result<()> {
        let base = Overrides::parse(dev, path, "profile.dev")?;
        self.base = std::mem::take(&mut self.base).merge(base);
        let panic = setting(
            dev,
            path,
            "profile.dev",
            "panic",
            "\"unwind\" or \"abort\"",
            |v| match v.as_str()? {
                "unwind" => Some(Panic::Unwind),
                "abort" => Some(Panic::Abort),
                _ => None,
            },
        )?;
        self.panic = panic.or(self.panic);
        if let Some(table) = dev.get("build-override").and_then(|t| t.as_table()) {
            let overrides = Overrides::parse(table, path, "profile.dev.build-override")?;
            self.build_override = std::mem::take(&mut self.build_override).merge(overrides);
        }
        for (spec, table) in dev
            .get("package")
            .and_then(|t| t.as_table())
            .into_iter()
            .flatten()
        {
            let Some(table) = table.as_table() else {
                continue;
            };
            let key = format!("profile.dev.package.{spec}");
            let overrides = Overrides::parse(table, path, &key)?;
            let merged = self
                .packages
                .remove(spec)
                .unwrap_or_default()
                .merge(overrides);
            self.packages.insert(spec.clone(), merged);
        }
        Ok(())
    }

    /// The settings that Cargo compiles the library, binaries, tests, and examples of a package
    /// with. `member` says whether the package is a member of the workspace.
    pub fn target(&self, name: &str, version: &str, member: bool) -> Settings {
        let mut settings = Settings::default();
        self.base.apply(&mut settings);
        self.apply_packages(&mut settings, name, version, member);
        settings
    }

    /// Whether the profile sets `panic = "abort"`. Cargo compiles a binary and its dependencies
    /// with the strategy, and tests, build scripts, and procedural macros with `unwind`. The
    /// strategy applies to the whole profile, because Cargo rejects `panic` in the package and
    /// `build-override` tables.
    pub fn aborts_on_panic(&self) -> bool {
        self.panic == Some(Panic::Abort)
    }

    /// The settings that Cargo compiles the build script and the procedural macro of a package
    /// with. Cargo compiles them without optimization and debug information unless
    /// `build-override` or a package table sets them.
    pub fn host(&self, name: &str, version: &str, member: bool) -> Settings {
        let mut settings = Settings::default();
        self.base.apply(&mut settings);
        settings.opt_level = OptLevel::O0;
        settings.debuginfo = DebugInfo::None;
        settings.codegen_units = None;
        self.build_override.apply(&mut settings);
        self.apply_packages(&mut settings, name, version, member);
        settings
    }

    /// Applies `[profile.dev.package."*"]` to a package outside the workspace, then the tables
    /// that name the package, without and then with its version.
    fn apply_packages(&self, settings: &mut Settings, name: &str, version: &str, member: bool) {
        if !member && let Some(overrides) = self.packages.get("*") {
            overrides.apply(settings);
        }
        for spec in [
            name.to_owned(),
            format!("{name}:{version}"),
            format!("{name}@{version}"),
        ] {
            if let Some(overrides) = self.packages.get(&spec) {
                overrides.apply(settings);
            }
        }
    }
}

/// Settings are the profile settings of one compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    opt_level: OptLevel,
    debuginfo: DebugInfo,
    debug_assertions: bool,
    overflow_checks: bool,
    codegen_units: Option<u32>,
}

impl Default for Settings {
    /// The settings of Cargo's built-in `dev` profile.
    fn default() -> Settings {
        Settings {
            opt_level: OptLevel::O0,
            debuginfo: DebugInfo::Full,
            debug_assertions: true,
            overflow_checks: true,
            codegen_units: None,
        }
    }
}

impl Settings {
    /// The flags that apply the settings to a crate.
    pub fn rustc_flags(&self) -> Vec<String> {
        let on_off = |on: bool| if on { "on" } else { "off" };
        let mut flags = vec![
            format!("-Copt-level={}", self.opt_level.as_str()),
            format!("-Cdebuginfo={}", self.debuginfo.as_str()),
            format!("-Cdebug-assertions={}", on_off(self.debug_assertions)),
            format!("-Coverflow-checks={}", on_off(self.overflow_checks)),
        ];
        if let Some(codegen_units) = self.codegen_units {
            flags.push(format!("-Ccodegen-units={codegen_units}"));
        }
        flags
    }

    /// The variables that Cargo sets for the build script of a crate compiled with these
    /// settings.
    pub fn script_env(&self) -> Vec<(String, Value)> {
        vec![
            ("OPT_LEVEL".to_owned(), Value::str(self.opt_level.as_str())),
            (
                "DEBUG".to_owned(),
                Value::str(if self.debuginfo == DebugInfo::None {
                    "false"
                } else {
                    "true"
                }),
            ),
            ("PROFILE".to_owned(), Value::str("debug")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, contents: &str) -> ConfigFile {
        ConfigFile {
            path: path.to_owned(),
            contents: contents.to_owned(),
        }
    }

    fn manifest(contents: &str) -> ConfigFile {
        file("Cargo.toml", contents)
    }

    #[test]
    fn the_default_profile_is_cargos_dev_profile() {
        let profile = Profile::parse(&manifest("[workspace]\n"), &[]).unwrap();
        assert_eq!(
            profile.target("serde", "1.0.0", false).rustc_flags(),
            [
                "-Copt-level=0",
                "-Cdebuginfo=2",
                "-Cdebug-assertions=on",
                "-Coverflow-checks=on"
            ]
        );
        assert!(!profile.aborts_on_panic());
        assert_eq!(
            profile.host("serde_derive", "1.0.0", false).rustc_flags(),
            [
                "-Copt-level=0",
                "-Cdebuginfo=0",
                "-Cdebug-assertions=on",
                "-Coverflow-checks=on"
            ]
        );
    }

    #[test]
    fn the_dev_table_applies_to_every_target() {
        let profile = Profile::parse(
            &manifest(
                r#"
[profile.dev]
opt-level = 1
debug = "line-tables-only"
debug-assertions = false
panic = "abort"
codegen-units = 4
lto = "off"
"#,
            ),
            &[],
        )
        .unwrap();
        let target = profile.target("app", "0.1.0", true);
        assert_eq!(
            target.rustc_flags(),
            [
                "-Copt-level=1",
                "-Cdebuginfo=line-tables-only",
                "-Cdebug-assertions=off",
                "-Coverflow-checks=on",
                "-Ccodegen-units=4"
            ]
        );
        assert!(profile.aborts_on_panic());
        assert_eq!(
            target.script_env(),
            [
                ("OPT_LEVEL".to_owned(), Value::str("1")),
                ("DEBUG".to_owned(), Value::str("true")),
                ("PROFILE".to_owned(), Value::str("debug")),
            ]
        );

        // Build scripts and procedural macros compile without optimization or debug information,
        // and keep the assertions of the profile.
        let host = profile.host("app", "0.1.0", true);
        assert_eq!(
            host.rustc_flags(),
            [
                "-Copt-level=0",
                "-Cdebuginfo=0",
                "-Cdebug-assertions=off",
                "-Coverflow-checks=on"
            ]
        );
    }

    #[test]
    fn package_tables_override_in_cargos_order() {
        let profile = Profile::parse(
            &manifest(
                r#"
[profile.dev.build-override]
opt-level = 2

[profile.dev.package."*"]
opt-level = 3

[profile.dev.package.app]
debug = false

[profile.dev.package."serde@1.0.0"]
opt-level = "s"
"#,
            ),
            &[],
        )
        .unwrap();
        // `*` applies only outside the workspace.
        assert!(
            profile
                .target("app", "0.1.0", true)
                .rustc_flags()
                .starts_with(&["-Copt-level=0".to_owned(), "-Cdebuginfo=0".to_owned()])
        );
        assert!(
            profile
                .target("rand", "0.8.0", false)
                .rustc_flags()
                .starts_with(&["-Copt-level=3".to_owned()])
        );
        assert!(
            profile
                .target("serde", "1.0.0", false)
                .rustc_flags()
                .starts_with(&["-Copt-level=s".to_owned()])
        );
        assert!(
            profile
                .target("serde", "1.0.1", false)
                .rustc_flags()
                .starts_with(&["-Copt-level=3".to_owned()])
        );
        assert!(
            profile
                .host("app", "0.1.0", true)
                .rustc_flags()
                .starts_with(&["-Copt-level=2".to_owned()])
        );
    }

    #[test]
    fn configuration_takes_precedence_over_the_manifest() {
        let profile = Profile::parse(
            &manifest("[profile.dev]\nopt-level = 1\ndebug = false\n"),
            &[
                file("home/.cargo/config.toml", "[profile.dev]\nopt-level = 2\n"),
                file(".cargo/config.toml", "[profile.dev]\nopt-level = 3\n"),
            ],
        )
        .unwrap();
        assert!(
            profile
                .target("app", "0.1.0", true)
                .rustc_flags()
                .starts_with(&["-Copt-level=3".to_owned(), "-Cdebuginfo=0".to_owned()])
        );
    }

    #[test]
    fn invalid_values_name_their_key_and_file() {
        let err = Profile::parse(&manifest("[profile.dev]\nopt-level = 4\n"), &[]).unwrap_err();
        assert!(err.to_string().contains("`profile.dev.opt-level`"), "{err}");
        assert!(err.to_string().contains("`Cargo.toml`"), "{err}");

        let err = Profile::parse(
            &manifest(""),
            &[file(
                ".cargo/config.toml",
                "[profile.dev.package.app]\ndebug-assertions = 1\n",
            )],
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("`profile.dev.package.app.debug-assertions`"),
            "{err}"
        );
    }
}
