/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

/// is_buck2_exe reports whether `path` names a yak executable (`yak`, `yak-daemon`, or the
/// executable of the current process).
pub(crate) fn is_buck2_exe(path: &Path) -> bool {
    let Some(file_stem) = path.file_stem() else {
        return false;
    };
    // On linux when the running executable is deleted or unlinked the string ' (deleted)' is appended to symlinked file in /proc/<pid>/exe
    if [
        OsStr::new("yak"),
        OsStr::new("yak (deleted)"),
        OsStr::new("yak-daemon"),
        OsStr::new("yak-daemon (deleted)"),
    ]
    .contains(&file_stem)
    {
        return true;
    }
    static CURRENT_EXE: OnceLock<PathBuf> = OnceLock::new();
    CURRENT_EXE
        .get_or_try_init(env::current_exe)
        .ok()
        .and_then(|current_exe| current_exe.file_stem())
        .is_some_and(|current_exe_file_stem| current_exe_file_stem == file_stem)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    use crate::is_buck2::is_buck2_exe;

    #[test]
    fn test_is_buck2_exe() {
        let (fake_buck, other_path) = if cfg!(windows) {
            ("C:\\dir\\yak.exe", "C:\\dir\\other.exe")
        } else {
            ("/dir/yak", "/dir/other")
        };

        assert!(is_buck2_exe(Path::new(fake_buck)));

        let current_exe = env::current_exe().unwrap();

        assert!(is_buck2_exe(&current_exe));

        assert!(!is_buck2_exe(Path::new(other_path)));
    }
}
