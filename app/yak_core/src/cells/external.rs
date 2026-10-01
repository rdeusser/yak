/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use pagable::Pagable;
use yak_error::yak_error;

use crate::cells::name::CellName;
use crate::fs::project_rel_path::ProjectRelativePathBuf;

#[derive(Debug, Clone, Dupe, Allocative, PartialEq, Eq, Pagable)]
pub enum ExternalCellOrigin {
    Bundled(CellName),
    Git(GitCellSetup),
    Cargo(CargoCellSetup),
    Go(GoCellSetup),
}

/// CargoCellSetup configures a cell whose build files describe the third-party packages of a
/// Cargo workspace, as `cargo metadata` resolves them.
#[derive(
    Debug,
    derive_more::Display,
    Clone,
    Dupe,
    allocative::Allocative,
    PartialEq,
    Eq,
    Hash,
    Pagable
)]
#[display("cargo({})", manifest)]
pub struct CargoCellSetup {
    /// The workspace's `Cargo.toml`, relative to the project root.
    pub manifest: Arc<ProjectRelativePathBuf>,
}

/// GoCellSetup configures a cell whose build files describe the packages of a Go module and its
/// dependencies, as `go list` resolves them.
#[derive(
    Debug,
    derive_more::Display,
    Clone,
    Dupe,
    allocative::Allocative,
    PartialEq,
    Eq,
    Hash,
    Pagable
)]
#[display("go({})", module)]
pub struct GoCellSetup {
    /// The module's `go.mod`, relative to the project root.
    pub module: Arc<ProjectRelativePathBuf>,
}

#[derive(
    Debug,
    derive_more::Display,
    Clone,
    Dupe,
    allocative::Allocative,
    PartialEq,
    Eq,
    Hash,
    Pagable
)]
#[display("git({}, {})", git_origin, commit)]
pub struct GitCellSetup {
    pub git_origin: Arc<str>,
    // Guaranteed to be a valid commit hash
    pub commit: Arc<str>,
    pub object_format: Option<GitObjectFormat>,
}

impl fmt::Display for ExternalCellOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundled(cell) => write!(f, "bundled({cell})"),
            Self::Git(git) => write!(f, "{git}"),
            Self::Cargo(cargo) => write!(f, "{cargo}"),
            Self::Go(go) => write!(f, "{go}"),
        }
    }
}

#[derive(Debug, Display, Eq, PartialEq, Clone, Dupe, Hash, Allocative, Pagable)]
pub enum GitObjectFormat {
    #[display("sha1")]
    Sha1,
    #[display("sha256")]
    Sha256,
}

impl FromStr for GitObjectFormat {
    type Err = yak_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sha1" => Ok(GitObjectFormat::Sha1),
            "sha256" => Ok(GitObjectFormat::Sha256),
            _ => Err(yak_error!(
                yak_error::ErrorTag::Input,
                "object_format must be one of `sha1` or `sha256` (got: {})",
                &s,
            )),
        }
    }
}

impl GitObjectFormat {
    pub fn check(&self, s: &str) -> Result<(), yak_error::Error> {
        match self {
            Self::Sha1 => {
                if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                    Ok(())
                } else {
                    Err(yak_error!(
                        yak_error::ErrorTag::Input,
                        "not a valid SHA1 digest (got: {})",
                        &s,
                    ))
                }
            }
            Self::Sha256 => {
                if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                    Ok(())
                } else {
                    Err(yak_error!(
                        yak_error::ErrorTag::Input,
                        "not a valid SHA256 digest (got: {})",
                        &s,
                    ))
                }
            }
        }
    }
}
