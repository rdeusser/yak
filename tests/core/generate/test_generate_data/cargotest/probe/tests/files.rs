use std::fs;

#[test]
fn runs_in_the_package_directory() {
    assert!(fs::read_to_string("Cargo.toml").unwrap().contains("name = \"probe\""));
}

#[test]
fn reads_declared_test_data_by_a_relative_path() {
    assert_eq!(fs::read_to_string("../shared.txt").unwrap(), "shared\n");
}

#[test]
fn gets_the_package_directory_at_run_time() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    assert!(std::path::Path::new(&dir).join("Cargo.toml").exists());
}
