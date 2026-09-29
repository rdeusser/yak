/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fs::File;
use std::sync::OnceLock;

use object::Object;
use yak_error::BuckErrorContext;
use yak_error::ErrorTag;

/// Provides information about this yak version.
pub struct BuckVersion {
    version: String,
    internal_exe_hash: String,
}

impl BuckVersion {
    pub fn get() -> yak_error::Result<&'static BuckVersion> {
        static VERSION: OnceLock<yak_error::Result<BuckVersion>> = OnceLock::new();
        VERSION
            .get_or_init(Self::compute)
            .as_ref()
            .map_err(|err: &yak_error::Error| err.clone().tag([ErrorTag::BuckVersionError]))
    }

    pub fn get_unique_id() -> yak_error::Result<&'static str> {
        Ok(Self::get()?.unique_id())
    }

    pub fn get_version() -> yak_error::Result<&'static str> {
        Ok(Self::get()?.version())
    }

    pub fn get_version_for_clap() -> &'static str {
        Self::get_version().unwrap_or("<version unavailable>")
    }

    fn extract_unique_id(file: &object::File) -> Option<String> {
        if let Ok(Some(build_id)) = file.build_id() {
            Some(hex::encode(build_id))
        } else if let Ok(Some(uuid)) = file.mach_uuid() {
            Some(hex::encode(uuid))
        } else {
            None
        }
    }

    fn hash_binary(file: &mut File) -> yak_error::Result<String> {
        let mut blake3 = blake3::Hasher::new();
        std::io::copy(file, &mut blake3).buck_error_context("Error hashing binary")?;
        let hash = blake3.finalize();
        Ok(hash.to_hex().to_string())
    }

    fn compute() -> yak_error::Result<BuckVersion> {
        // Make sure to use the daemon exe's version, if there is one
        let exe = crate::daemon::client::connect::get_daemon_exe()
            .buck_error_context("Error finding daemon executable for versioning")?;

        let mut file = File::open(&exe).with_buck_error_context(|| {
            format!("Error opening daemon executable at {}", exe.display())
        })?;

        let file_m = unsafe { memmap2::Mmap::map(&file) }.with_buck_error_context(|| {
            format!(
                "Error to mmap daemon executable at {} for versioning",
                exe.display()
            )
        })?;

        let file_object = object::File::parse(&*file_m).map_err(|e| {
            yak_error::yak_error!(
                yak_error::ErrorTag::Tier0,
                "Error parsing daemon executable at {} for versioning: {e:#}",
                exe.display()
            )
        })?;

        let (internal_exe_hash, internal_exe_hash_kind) =
            if let Some(internal_exe_hash) = Self::extract_unique_id(&file_object) {
                (internal_exe_hash, "<build-id>")
            } else {
                (Self::hash_binary(&mut file)?, "<exe-hash>")
            };

        let version = if let Some(version) = yak_build_info::revision() {
            version.to_owned()
        } else {
            format!("{internal_exe_hash} {internal_exe_hash_kind}")
        };

        Ok(BuckVersion {
            version,
            internal_exe_hash,
        })
    }

    /// Provides a globally unique identifier for this yak executable.
    pub fn unique_id(&self) -> &str {
        &self.internal_exe_hash
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}
