use std::env;
use std::path::PathBuf;

/// The profile directory that Cargo builds the package's targets into, found from the path of
/// the running test, which Cargo puts in its `deps` directory.
pub fn profile_dir() -> PathBuf {
    let test = env::current_exe().unwrap();
    test.parent().unwrap().parent().unwrap().to_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_a_workspace_file_at_run_time() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../shared.txt");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "shared
");
    }

    #[test]
    fn finds_the_example_library() {
        let name = format!(
            "{}plugin{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        let path = super::profile_dir().join("examples").join(name);
        assert!(path.exists(), "{} is missing", path.display());
    }
}
