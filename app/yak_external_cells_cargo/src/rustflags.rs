/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `rustflags` settings of Cargo's configuration files, which Cargo passes to every `rustc`
//! it runs.

use std::collections::BTreeMap;

use crate::cfg::PlatformCondition;
use crate::cfg::TargetCfg;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum RustFlagsError {
    #[error("Cargo configuration `{path}` is not valid TOML: {error}")]
    InvalidToml { path: String, error: String },
    #[error("`{key}` in Cargo configuration `{path}` is neither a string nor an array of strings")]
    InvalidValue { path: String, key: String },
    #[error(
        "`{key}` is a string in one Cargo configuration file and an array in another (`{path}`), which Cargo cannot merge"
    )]
    MixedTypes { path: String, key: String },
}

/// ConfigFile is one of Cargo's configuration files.
#[derive(Debug, Clone)]
pub struct ConfigFile {
    /// The path that errors name.
    pub path: String,
    pub contents: String,
}

/// A `rustflags` value: a string of flags separated by whitespace, or an array of flags.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FlagsValue {
    String(String),
    Array(Vec<String>),
}

impl FlagsValue {
    fn parse(value: &toml::Value, path: &str, key: &str) -> yak_error::Result<FlagsValue> {
        let invalid = || RustFlagsError::InvalidValue {
            path: path.to_owned(),
            key: key.to_owned(),
        };
        match value {
            toml::Value::String(s) => Ok(FlagsValue::String(s.clone())),
            toml::Value::Array(items) => Ok(FlagsValue::Array(
                items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned).ok_or_else(invalid))
                    .collect::<Result<_, _>>()?,
            )),
            _ => Err(invalid().into()),
        }
    }

    /// Merges `self` with `higher`, a value of a file with higher precedence. Cargo joins arrays,
    /// with the higher precedence items last, and takes the string of higher precedence.
    fn merge(self, higher: FlagsValue, path: &str, key: &str) -> yak_error::Result<FlagsValue> {
        match (self, higher) {
            (FlagsValue::Array(mut lower), FlagsValue::Array(higher)) => {
                lower.extend(higher);
                Ok(FlagsValue::Array(lower))
            }
            (FlagsValue::String(_), FlagsValue::String(higher)) => Ok(FlagsValue::String(higher)),
            _ => Err(RustFlagsError::MixedTypes {
                path: path.to_owned(),
                key: key.to_owned(),
            }
            .into()),
        }
    }

    fn into_flags(self) -> Vec<String> {
        match self {
            FlagsValue::String(s) => s.split_whitespace().map(str::to_owned).collect(),
            FlagsValue::Array(flags) => flags,
        }
    }
}

/// RustFlags holds the `build.rustflags` and `target.<triple>.rustflags` settings of Cargo's
/// configuration files, where `<triple>` can also be a `cfg(...)` expression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustFlags {
    build: Vec<String>,
    /// By the key of the `target` table, which sorts the `cfg(...)` keys as Cargo joins them.
    targets: BTreeMap<String, (PlatformCondition, Vec<String>)>,
}

impl RustFlags {
    /// Parses and merges `files`, which are ordered from the lowest precedence to the highest, as
    /// Cargo orders the configuration of `$CARGO_HOME` and of the directories from the root of
    /// the file system down to the workspace.
    pub fn parse(files: &[ConfigFile]) -> yak_error::Result<RustFlags> {
        let mut build: Option<FlagsValue> = None;
        let mut targets: BTreeMap<String, FlagsValue> = BTreeMap::new();
        let merge = |slot: Option<FlagsValue>, value: &toml::Value, path: &str, key: &str| {
            let value = FlagsValue::parse(value, path, key)?;
            match slot {
                None => yak_error::Ok(value),
                Some(lower) => lower.merge(value, path, key),
            }
        };
        for file in files {
            let path = file.path.as_str();
            let table: toml::Table =
                toml::from_str(&file.contents).map_err(|e| RustFlagsError::InvalidToml {
                    path: path.to_owned(),
                    error: e.to_string(),
                })?;
            if let Some(value) = table
                .get("build")
                .and_then(|b| b.as_table())
                .and_then(|b| b.get("rustflags"))
            {
                build = Some(merge(build.take(), value, path, "build.rustflags")?);
            }
            for (target, settings) in table
                .get("target")
                .and_then(|t| t.as_table())
                .into_iter()
                .flatten()
            {
                if let Some(value) = settings.as_table().and_then(|s| s.get("rustflags")) {
                    let key = format!("target.{target}.rustflags");
                    let merged = merge(targets.remove(target), value, path, &key)?;
                    targets.insert(target.clone(), merged);
                }
            }
        }
        Ok(RustFlags {
            build: build.map(FlagsValue::into_flags).unwrap_or_default(),
            targets: targets
                .into_iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        (PlatformCondition::parse(&key)?, value.into_flags()),
                    ))
                })
                .collect::<yak_error::Result<_>>()?,
        })
    }

    /// Reports whether some `target` setting is a `cfg(...)` expression, whose match depends on
    /// the cfg values that the flags themselves can change.
    pub fn has_cfg_targets(&self) -> bool {
        self.targets
            .values()
            .any(|(condition, _)| matches!(condition, PlatformCondition::Cfg(_)))
    }

    /// The flags that Cargo passes when it compiles for `triple`: the `target` settings that
    /// match it, the triple's first, or else `build.rustflags`. Without `cfg`, no `cfg(...)`
    /// setting matches.
    pub fn for_target(&self, triple: &str, cfg: Option<&TargetCfg>) -> Vec<String> {
        let mut flags = Vec::new();
        let mut matched = false;
        for (condition, target_flags) in self.targets.values() {
            if matches!(condition, PlatformCondition::Triple(t) if t == triple) {
                matched = true;
                flags.extend(target_flags.iter().cloned());
            }
        }
        for (condition, target_flags) in self.targets.values() {
            if let (PlatformCondition::Cfg(expr), Some(cfg)) = (condition, cfg)
                && expr.eval(cfg)
            {
                matched = true;
                flags.extend(target_flags.iter().cloned());
            }
        }
        if matched { flags } else { self.build.clone() }
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

    #[test]
    fn build_rustflags_apply_to_every_target() {
        let flags = RustFlags::parse(&[file(
            ".cargo/config.toml",
            "[build]\nrustflags = [\"--cfg\", \"tokio_unstable\"]\n",
        )])
        .unwrap();
        assert_eq!(
            flags.for_target("x86_64-unknown-linux-gnu", None),
            ["--cfg", "tokio_unstable"]
        );
        assert!(!flags.has_cfg_targets());
    }

    #[test]
    fn arrays_join_and_strings_take_the_higher_precedence() {
        let arrays = RustFlags::parse(&[
            file("home", "[build]\nrustflags = [\"-Ca\"]\n"),
            file("workspace", "[build]\nrustflags = [\"-Cb\"]\n"),
        ])
        .unwrap();
        assert_eq!(arrays.for_target("t", None), ["-Ca", "-Cb"]);

        let strings = RustFlags::parse(&[
            file("home", "[build]\nrustflags = \"-Ca\"\n"),
            file("workspace", "[build]\nrustflags = \"-Cb  -Cc\"\n"),
        ])
        .unwrap();
        assert_eq!(strings.for_target("t", None), ["-Cb", "-Cc"]);

        let mixed = RustFlags::parse(&[
            file("home", "[build]\nrustflags = \"-Ca\"\n"),
            file("workspace", "[build]\nrustflags = [\"-Cb\"]\n"),
        ]);
        assert!(mixed.unwrap_err().to_string().contains("`workspace`"));
    }

    #[test]
    fn target_settings_replace_build_rustflags() {
        let flags = RustFlags::parse(&[file(
            ".cargo/config.toml",
            r#"
[build]
rustflags = ["--cfg", "everywhere"]

[target.x86_64-unknown-linux-gnu]
rustflags = ["-Ctarget-cpu=native"]

[target.'cfg(target_os = "linux")']
rustflags = ["--cfg", "linux"]

[target.'cfg(windows)']
rustflags = ["--cfg", "windows"]
"#,
        )])
        .unwrap();
        assert!(flags.has_cfg_targets());
        let linux = TargetCfg::parse("target_os=\"linux\"\nunix\n").unwrap();
        let macos = TargetCfg::parse("target_os=\"macos\"\nunix\n").unwrap();
        assert_eq!(
            flags.for_target("x86_64-unknown-linux-gnu", Some(&linux)),
            ["-Ctarget-cpu=native", "--cfg", "linux"]
        );
        assert_eq!(
            flags.for_target("x86_64-unknown-linux-gnu", None),
            ["-Ctarget-cpu=native"]
        );
        assert_eq!(
            flags.for_target("aarch64-unknown-linux-gnu", Some(&linux)),
            ["--cfg", "linux"]
        );
        assert_eq!(
            flags.for_target("aarch64-apple-darwin", Some(&macos)),
            ["--cfg", "everywhere"]
        );
    }

    #[test]
    fn invalid_values_name_their_file() {
        let err = RustFlags::parse(&[file("cfg.toml", "[build]\nrustflags = 1\n")]).unwrap_err();
        assert!(err.to_string().contains("`build.rustflags`"));
        assert!(err.to_string().contains("`cfg.toml`"));
    }
}
