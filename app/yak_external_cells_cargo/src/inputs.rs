/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The files whose changes can change the output of `cargo metadata`, from which a cargo cell
//! generates its files.

/// Reports whether a change to `path`, a `/`-separated path relative to the project root, can
/// change the output of `cargo metadata`. `added_or_removed` says whether the file appeared or
/// disappeared, as opposed to changing its contents.
///
/// Cargo reads each `Cargo.toml` and `Cargo.lock`, the `.cargo/config.toml` files of the
/// workspace directory and its parents, and the `rust-toolchain.toml` that selects the Cargo that
/// rustup runs. Cargo finds a package's targets by which of `build.rs`, `src/lib.rs`,
/// `src/main.rs`, and the files in `src/bin`, `tests`, `examples`, and `benches` exist. The
/// package directory is unknown here, so a file matches a target pattern in any directory.
pub fn is_metadata_input(path: &str, added_or_removed: bool) -> bool {
    let components: Vec<&str> = path.split('/').collect();
    let Some((&name, dirs)) = components.split_last() else {
        return false;
    };
    let parent = dirs.last().copied();
    if matches!(
        name,
        "Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
    ) || (parent == Some(".cargo") && matches!(name, "config" | "config.toml"))
    {
        return true;
    }
    if !added_or_removed || !name.ends_with(".rs") {
        return false;
    }
    let ends_with = |suffix: &[&str]| dirs.ends_with(suffix);
    let target_dir = |dir: Option<&&str>| matches!(dir, Some(&"tests" | &"examples" | &"benches"));
    name == "build.rs"
        || (ends_with(&["src"]) && matches!(name, "lib.rs" | "main.rs"))
        || ends_with(&["src", "bin"])
        || (name == "main.rs"
            && dirs.len() >= 3
            && dirs[..dirs.len() - 1].ends_with(&["src", "bin"]))
        || target_dir(dirs.last())
        || (name == "main.rs" && dirs.len() >= 2 && target_dir(dirs.get(dirs.len() - 2)))
}

#[cfg(test)]
mod tests {
    use super::is_metadata_input;

    #[test]
    fn manifests_and_configuration_count_for_any_change() {
        for path in [
            "Cargo.toml",
            "crates/a/Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
            "crates/.cargo/config",
            "rust-toolchain.toml",
        ] {
            assert!(is_metadata_input(path, false), "{path}");
        }
    }

    #[test]
    fn target_files_count_when_added_or_removed() {
        for path in [
            "build.rs",
            "crates/a/build.rs",
            "crates/a/src/lib.rs",
            "src/main.rs",
            "crates/a/src/bin/tool.rs",
            "crates/a/src/bin/tool/main.rs",
            "crates/a/tests/cli.rs",
            "crates/a/tests/cli/main.rs",
            "examples/demo.rs",
            "benches/speed.rs",
        ] {
            assert!(is_metadata_input(path, true), "{path}");
            assert!(!is_metadata_input(path, false), "{path}");
        }
    }

    #[test]
    fn other_sources_do_not_count() {
        for path in [
            "crates/a/src/parse.rs",
            "crates/a/src/bin/tool/args.rs",
            "crates/a/tests/cli/helpers.rs",
            "crates/a/tests/data/input.txt",
            "README.md",
            "config.toml",
        ] {
            assert!(!is_metadata_input(path, true), "{path}");
        }
    }
}
