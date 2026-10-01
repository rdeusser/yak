/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Translates `go list` output into the files of a go cell. The daemon side of the cell, which
//! runs Go and serves the files, is in `yak_external_cells`.

mod generate;
mod header;
pub mod list;
mod platform;

#[cfg(test)]
mod tests;

pub use generate::GoCell;
pub use generate::ModuleInputs;
pub use generate::ModuleVersion;
pub use generate::generate;
pub use header::go_file_header;
pub use header::is_cell_input;
pub use platform::GoPlatform;
pub use platform::PLATFORMS;
