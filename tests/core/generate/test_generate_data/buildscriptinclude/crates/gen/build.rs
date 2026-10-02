use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // A path relative to the package's directory, where the script runs, as
    // protobuf build scripts name the `.proto` files of sibling crates.
    let relative = fs::read_to_string("../../proto/api.txt").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let absolute = fs::read_to_string(Path::new(&manifest_dir).join("../../proto/api.txt")).unwrap();
    assert_eq!(relative, absolute);
    println!("cargo::rerun-if-changed=../../proto/api.txt");
    println!("cargo::rustc-env=API={}", relative.trim());
}
