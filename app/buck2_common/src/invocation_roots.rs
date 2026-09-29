/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::LazyLock;

use allocative::Allocative;
use buck2_core::buck2_env;
use buck2_core::fs::project::ProjectRoot;
use buck2_core::fs::project_rel_path::ProjectRelativePathBuf;
use buck2_error::BuckErrorContext;
use buck2_error::BuckErrorOptionContext;
use buck2_fs::fs_util;
use buck2_fs::paths::abs_norm_path::AbsNormPath;
use buck2_fs::paths::abs_norm_path::AbsNormPathBuf;
use buck2_fs::paths::abs_path::AbsPathBuf;
use buck2_fs::paths::file_name::FileName;
use buck2_fs::paths::file_name::FileNameBuf;
use buck2_fs::working_dir::AbsWorkingDir;

use crate::invocation_paths::InvocationPaths;
use crate::invocation_paths_result::InvocationPathsResult;

#[derive(Debug, buck2_error::Error)]
enum BuckCliError {
    #[error(
        "Couldn't find a project root for directory `{}`. Expected to find a .yakconfig file.", _0.path().display()
    )]
    #[buck2(tag = NoBuckRoot)]
    NoBuckRoot(AbsWorkingDir),
}

#[derive(Clone, Allocative)]
pub struct InvocationRoots {
    pub project_root: ProjectRoot,
    pub cwd: ProjectRelativePathBuf,
}

impl InvocationRoots {
    pub fn common_buckd_dir(&self) -> buck2_error::Result<AbsNormPathBuf> {
        Ok(home_buck_dir()?.join(FileName::unchecked_new("yakd")))
    }

    pub fn paranoid_info_path(&self) -> buck2_error::Result<AbsPathBuf> {
        // Used in tests
        if let Some(p) = buck2_env!("YAK_PARANOID_PATH")? {
            return AbsPathBuf::try_from(p.to_owned());
        }

        Ok(self
            .common_buckd_dir()?
            .join(FileName::new("paranoid.info")?)
            .into_abs_path_buf())
    }
}

/// Finds the project root.
///
/// This uses a rather liberal definition of "roots". It traverses the directory and its parents
/// looking for all .yakconfig files and the furthest one (with the shortest path) will be detected
/// as the "project root".
///
/// We also look for .yakroot files, and if we find one of them, we don't traverse further upwards.
/// The contents of the .yakroot file is entirely ignored.
fn get_roots(from: &AbsWorkingDir) -> buck2_error::Result<Option<InvocationRoots>> {
    let mut project_root = None;

    let home_dir = dirs::home_dir();
    for curr in from.path().ancestors() {
        if fs_util::try_exists(curr.join(FileName::unchecked_new(".yakconfig")))? {
            // Do not allow /home/{unixname}, /home or / to be a cell,
            // and /home/{unixname}/.yakconfig is used for config override
            if let Some(home_dir_path) = &home_dir {
                if home_dir_path == curr.as_path() {
                    break;
                }
            }
            project_root = Some(curr.to_owned());
        }

        if fs_util::try_exists(curr.join(FileName::unchecked_new(".yakroot")))? {
            break;
        }
    }

    #[allow(clippy::manual_map)]
    Ok(match project_root {
        Some(project_root) => {
            let rel_cwd = from
                .path()
                .strip_prefix(&project_root)
                .expect("By construction")
                .into_owned();
            Some(InvocationRoots {
                project_root: ProjectRoot::new_unchecked(project_root),
                cwd: rel_cwd.into(),
            })
        }
        None => None,
    })
}

pub fn find_invocation_roots(from: &AbsWorkingDir) -> buck2_error::Result<InvocationRoots> {
    get_roots(from)?.ok_or_else(|| BuckCliError::NoBuckRoot(from.to_owned()).into())
}

pub fn get_invocation_paths_result(
    from: &AbsWorkingDir,
    isolation: FileNameBuf,
) -> InvocationPathsResult {
    match get_roots(from) {
        Ok(Some(roots)) => InvocationPathsResult::Paths(InvocationPaths { roots, isolation }),
        Ok(None) => {
            InvocationPathsResult::OutsideOfRepo(BuckCliError::NoBuckRoot(from.to_owned()).into())
        }
        Err(e) => InvocationPathsResult::OtherError(e),
    }
}

/// `~/.yak`.
/// TODO(cjhopman): We currently place all yakd info into a directory owned by the user.
/// This is broken when multiple users try to share the same checkout.
///
/// A daemon shared across users would be a privilege escalation vulnerability, because
/// `yak run` runs whatever command the daemon returns.
///
/// There's a couple ways we could resolve this:
///
/// 1. Use a shared .yakd information directory and have the client verify the identity of
///    the server before doing anything with it. If the identity is different, kill it and
///    start a new one.
///
/// 2. Keep user-owned .yakd directory, use some other mechanism to move ownership of
///    output directories between different yakd instances.
pub(crate) fn home_buck_dir() -> buck2_error::Result<&'static AbsNormPath> {
    fn find_dir() -> buck2_error::Result<AbsNormPathBuf> {
        let home = buck2_wrapper_common::buck2_home_dir()
            .internal_error("Expected a HOME directory to be available")?;
        let home =
            AbsNormPathBuf::new(home).buck_error_context("Expected an absolute HOME directory")?;
        Ok(home.join(FileName::new(".yak")?))
    }

    static DIR: LazyLock<buck2_error::Result<AbsNormPathBuf>> = LazyLock::new(find_dir);

    Ok(LazyLock::force(&DIR).as_ref().map_err(dupe::Dupe::dupe)?)
}
