/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The package graph of `cargo metadata`, and the dependencies and variables that both the
//! third-party build file and the workspace members take from it.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::cfg::PlatformCondition;
use crate::cfg::TargetCfg;
use crate::metadata::DepKind;
use crate::metadata::Metadata;
use crate::metadata::Node;
use crate::metadata::Package;
use crate::metadata::Target;
use crate::starlark::Value;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
pub(crate) enum GenerateError {
    #[error(
        "`{0}` is a path dependency outside the workspace. Add it to the workspace's `members`."
    )]
    PathOutsideWorkspace(String),
    #[error(
        "Two packages of the dependency graph have the directory `{0}` in the cargo cell, which keeps one copy of sources per directory"
    )]
    DuplicatePackage(String),
    #[error("`cargo metadata` names package `{0}`, which it does not describe")]
    UnknownPackage(String),
    #[error("Package `{0}` has no library, but `{1}` depends on it")]
    NoLibrary(String, String),
    #[error("`{0}` is not under the directory `{1}`")]
    OutsideDirectory(String, String),
}

/// CargoPlatform is one of the platform names that `prelude/rust/cargo_package.bzl` selects on,
/// with the target triple and cfg values it stands for.
#[derive(Debug, Clone)]
pub struct CargoPlatform {
    pub name: String,
    pub triple: String,
    pub cfg: TargetCfg,
}

/// DEFAULT_PLATFORMS pairs the names of `DEFAULT_CARGO_PLATFORMS` in
/// `prelude/rust/cargo_package.bzl` with their target triples.
pub const DEFAULT_PLATFORMS: &[(&str, &str)] = &[
    ("linux-arm64", "aarch64-unknown-linux-gnu"),
    ("linux-riscv64", "riscv64gc-unknown-linux-gnu"),
    ("linux-x86_64", "x86_64-unknown-linux-gnu"),
    ("macos-arm64", "aarch64-apple-darwin"),
    ("macos-x86_64", "x86_64-apple-darwin"),
    ("wasi", "wasm32-wasip1"),
    ("wasm32", "wasm32-unknown-unknown"),
    ("windows-gnu", "x86_64-pc-windows-gnu"),
    ("windows-msvc", "x86_64-pc-windows-msvc"),
];

/// The target name of a third-party package, which is unique in the cell.
pub(crate) fn target_id(p: &Package) -> String {
    format!("{}-{}", p.name, p.version)
}

pub(crate) fn crate_name(name: &str) -> String {
    name.replace('-', "_")
}

pub(crate) fn lib_target(p: &Package) -> Option<&Target> {
    p.targets.iter().find(|t| t.is_lib())
}

/// Where a dependency applies: on every platform, or on the named ones.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Platforms {
    All,
    Some(BTreeSet<String>),
}

fn applies_on(kinds: &[&DepKind], platforms: &[CargoPlatform]) -> yak_error::Result<Platforms> {
    let mut names = BTreeSet::new();
    for kind in kinds {
        let Some(target) = &kind.target else {
            return Ok(Platforms::All);
        };
        let condition = PlatformCondition::parse(target)?;
        for p in platforms {
            if condition.matches(&p.triple, &p.cfg) {
                names.insert(p.name.clone());
            }
        }
    }
    if names.len() == platforms.len() {
        Ok(Platforms::All)
    } else {
        Ok(Platforms::Some(names))
    }
}

/// Deps holds the dependencies of one kind (normal, build, or dev) of one package, split into
/// those that apply on every platform and those that apply on some.
#[derive(Default)]
pub(crate) struct Deps {
    pub(crate) deps: Vec<String>,
    pub(crate) named_deps: Vec<(String, String)>,
    platform_deps: BTreeMap<String, Vec<String>>,
    platform_named_deps: BTreeMap<String, Vec<(String, String)>>,
}

impl Deps {
    fn add(&mut self, extern_name: Option<String>, label: String, platforms: Platforms) {
        match (platforms, extern_name) {
            (Platforms::All, None) => self.deps.push(label),
            (Platforms::All, Some(name)) => self.named_deps.push((name, label)),
            (Platforms::Some(names), None) => {
                for name in names {
                    self.platform_deps
                        .entry(name)
                        .or_default()
                        .push(label.clone());
                }
            }
            (Platforms::Some(names), Some(extern_name)) => {
                for name in names {
                    self.platform_named_deps
                        .entry(name)
                        .or_default()
                        .push((extern_name.clone(), label.clone()));
                }
            }
        }
    }

    /// The dependencies of both `self` and `other`, as a test takes its normal and dev
    /// dependencies together.
    pub(crate) fn merged(&self, other: &Deps) -> Deps {
        let mut merged = Deps {
            deps: self.deps.clone(),
            named_deps: self.named_deps.clone(),
            platform_deps: self.platform_deps.clone(),
            platform_named_deps: self.platform_named_deps.clone(),
        };
        merged.deps.extend(other.deps.iter().cloned());
        merged.named_deps.extend(other.named_deps.iter().cloned());
        for (name, deps) in &other.platform_deps {
            merged
                .platform_deps
                .entry(name.clone())
                .or_default()
                .extend(deps.iter().cloned());
        }
        for (name, deps) in &other.platform_named_deps {
            merged
                .platform_named_deps
                .entry(name.clone())
                .or_default()
                .extend(deps.iter().cloned());
        }
        merged.deps.sort();
        merged.deps.dedup();
        merged
    }

    pub(crate) fn deps_value(&self) -> Value {
        Value::strs(self.deps.iter().cloned())
    }

    pub(crate) fn named_deps_value(&self) -> Value {
        named_deps_value(&self.named_deps)
    }

    /// The `platform` attribute of the Cargo macros of `prelude/rust/cargo_package.bzl`.
    pub(crate) fn platform_value(&self) -> Value {
        let names: BTreeSet<&String> = self
            .platform_deps
            .keys()
            .chain(self.platform_named_deps.keys())
            .collect();
        Value::Dict(
            names
                .into_iter()
                .map(|name| {
                    let mut attrs = Vec::new();
                    if let Some(deps) = self.platform_deps.get(name) {
                        attrs.push(("deps".to_owned(), Value::strs(deps.iter().cloned())));
                    }
                    if let Some(named) = self.platform_named_deps.get(name) {
                        attrs.push(("named_deps".to_owned(), named_deps_value(named)));
                    }
                    (name.clone(), Value::Dict(attrs))
                })
                .collect(),
        )
    }
}

fn named_deps_value(named: &[(String, String)]) -> Value {
    Value::Dict(
        named
            .iter()
            .map(|(name, label)| (name.clone(), Value::str(label)))
            .collect(),
    )
}

/// NodeDeps holds the dependencies of a package by kind.
#[derive(Default)]
pub(crate) struct NodeDeps {
    pub(crate) normal: Deps,
    pub(crate) build: Deps,
    pub(crate) dev: Deps,
}

pub(crate) struct Graph<'a> {
    packages: HashMap<&'a str, &'a Package>,
    members: HashSet<&'a str>,
}

impl<'a> Graph<'a> {
    pub(crate) fn new(metadata: &'a Metadata) -> Graph<'a> {
        Graph {
            packages: metadata
                .packages
                .iter()
                .map(|p| (p.id.as_str(), p))
                .collect(),
            members: metadata
                .workspace_members
                .iter()
                .map(String::as_str)
                .collect(),
        }
    }

    pub(crate) fn package(&self, id: &str) -> yak_error::Result<&'a Package> {
        self.packages
            .get(id)
            .copied()
            .ok_or_else(|| GenerateError::UnknownPackage(id.to_owned()).into())
    }

    pub(crate) fn is_member(&self, id: &str) -> bool {
        self.members.contains(id)
    }

    /// The dependencies of `node` by kind. `label` names the library target of a dependency.
    pub(crate) fn deps(
        &self,
        node: &Node,
        platforms: &[CargoPlatform],
        label: impl Fn(&Package) -> yak_error::Result<String>,
    ) -> yak_error::Result<NodeDeps> {
        let dependent = self.package(&node.id)?;
        let mut deps = NodeDeps::default();
        for dep in &node.deps {
            let package = self.package(&dep.pkg)?;
            let lib = lib_target(package).ok_or_else(|| {
                GenerateError::NoLibrary(target_id(package), target_id(dependent))
            })?;
            let label = label(package)?;
            let extern_name = (dep.name != crate_name(&lib.name)).then(|| dep.name.clone());
            for (kind, into) in [
                (None, &mut deps.normal),
                (Some("build"), &mut deps.build),
                (Some("dev"), &mut deps.dev),
            ] {
                let kinds: Vec<&DepKind> = dep
                    .dep_kinds
                    .iter()
                    .filter(|k| k.kind.as_deref() == kind)
                    .collect();
                if !kinds.is_empty() {
                    into.add(
                        extern_name.clone(),
                        label.clone(),
                        applies_on(&kinds, platforms)?,
                    );
                }
            }
        }
        Ok(deps)
    }
}

/// The `CARGO_PKG_*` variables that Cargo sets when it compiles a package.
pub(crate) fn package_env(package: &Package) -> Vec<(String, Value)> {
    let (release, pre) = match package.version.split_once('-') {
        Some((release, pre)) => (release, pre),
        None => (package.version.as_str(), ""),
    };
    let mut parts = release.split('.');
    let mut part = || parts.next().unwrap_or("").to_owned();
    let (major, minor, patch) = (part(), part(), part());
    let mut env = vec![
        ("CARGO_CRATE_NAME", crate_name(&package.name)),
        ("CARGO_PKG_NAME", package.name.clone()),
        ("CARGO_PKG_VERSION", package.version.clone()),
        ("CARGO_PKG_VERSION_MAJOR", major),
        ("CARGO_PKG_VERSION_MINOR", minor),
        ("CARGO_PKG_VERSION_PATCH", patch),
        ("CARGO_PKG_VERSION_PRE", pre.to_owned()),
        ("CARGO_PKG_AUTHORS", package.authors.join(":")),
        (
            "CARGO_PKG_DESCRIPTION",
            package.description.clone().unwrap_or_default(),
        ),
        (
            "CARGO_PKG_HOMEPAGE",
            package.homepage.clone().unwrap_or_default(),
        ),
        (
            "CARGO_PKG_REPOSITORY",
            package.repository.clone().unwrap_or_default(),
        ),
        (
            "CARGO_PKG_LICENSE",
            package.license.clone().unwrap_or_default(),
        ),
    ];
    if let Some(links) = &package.links {
        env.push(("CARGO_MANIFEST_LINKS", links.clone()));
    }
    env.into_iter()
        .map(|(k, v)| (k.to_owned(), Value::Str(v)))
        .collect()
}

/// The path of `path` relative to `dir`, with forward slashes.
pub(crate) fn relative_to(dir: &std::path::Path, path: &str) -> yak_error::Result<String> {
    let rel = std::path::Path::new(path)
        .strip_prefix(dir)
        .map_err(|_| GenerateError::OutsideDirectory(path.to_owned(), dir.display().to_string()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}
