/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The parts of `cargo metadata --format-version 1` output that the cell reads.

use serde::Deserialize;

/// Metadata is the output of `cargo metadata --format-version 1`.
#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
    pub resolve: Resolve,
}

/// WorkspaceLayout is the output of `cargo metadata --no-deps --format-version 1`, which
/// describes the workspace members without resolving their dependencies.
#[derive(Debug, Deserialize)]
pub struct WorkspaceLayout {
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
    pub workspace_root: String,
}

impl WorkspaceLayout {
    pub fn parse(json: &str) -> yak_error::Result<WorkspaceLayout> {
        Ok(serde_json::from_str(json)?)
    }

    /// The directories of the workspace members, relative to the workspace root, with forward
    /// slashes and sorted. The directory of a member at the root is empty.
    pub fn member_dirs(&self) -> yak_error::Result<Vec<String>> {
        let root = std::path::Path::new(&self.workspace_root);
        let mut dirs = self
            .packages
            .iter()
            .filter(|p| self.workspace_members.contains(&p.id))
            .map(|p| {
                let dir = std::path::Path::new(&p.manifest_path)
                    .parent()
                    .unwrap_or(std::path::Path::new(""));
                crate::graph::relative_to(root, &dir.to_string_lossy())
            })
            .collect::<yak_error::Result<Vec<_>>>()?;
        dirs.sort();
        Ok(dirs)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Where the package comes from, such as
    /// `registry+https://github.com/rust-lang/crates.io-index`. It is `None` for a path
    /// dependency.
    pub source: Option<String>,
    pub manifest_path: String,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub links: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub name: String,
    /// Such as `lib`, `rlib`, `proc-macro`, `bin`, `test`, or `custom-build`.
    pub kind: Vec<String>,
    pub src_path: String,
    pub edition: String,
}

impl Target {
    pub fn is_lib(&self) -> bool {
        self.kind
            .iter()
            .any(|k| matches!(k.as_str(), "lib" | "rlib" | "dylib" | "proc-macro"))
    }

    pub fn is_proc_macro(&self) -> bool {
        self.kind.iter().any(|k| k == "proc-macro")
    }

    pub fn is_build_script(&self) -> bool {
        self.kind.iter().any(|k| k == "custom-build")
    }
}

#[derive(Debug, Deserialize)]
pub struct Resolve {
    pub nodes: Vec<Node>,
}

/// Node is one package of the resolved graph, with the features that Cargo enables on it.
#[derive(Debug, Deserialize)]
pub struct Node {
    pub id: String,
    pub deps: Vec<NodeDep>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NodeDep {
    /// The name the dependent uses for the dependency's library, after any rename.
    pub name: String,
    pub pkg: String,
    pub dep_kinds: Vec<DepKind>,
}

#[derive(Debug, Deserialize)]
pub struct DepKind {
    /// `None` for a normal dependency, or `build` or `dev`.
    pub kind: Option<String>,
    /// The platform condition, such as `cfg(unix)`, or `None` for every platform.
    pub target: Option<String>,
}

impl Metadata {
    pub fn parse(json: &str) -> yak_error::Result<Metadata> {
        Ok(serde_json::from_str(json)?)
    }
}
