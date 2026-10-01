/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Translates `cargo metadata` output into the build files of a cargo cell. The daemon side of
//! the cell, which runs Cargo and serves the files, is in `yak_external_cells`.

pub mod cfg;
mod graph;
mod inputs;
pub mod metadata;
mod third_party;
mod workspace;

#[cfg(test)]
mod tests;

pub use graph::CargoPlatform;
pub use graph::DEFAULT_PLATFORMS;
pub use inputs::is_metadata_input;
pub use third_party::ThirdParty;
pub use third_party::ThirdPartyPackage;
pub use third_party::generate_third_party;
pub use workspace::generate_workspace;
