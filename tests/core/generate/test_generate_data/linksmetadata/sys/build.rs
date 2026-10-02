use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // A directory of generated headers in `OUT_DIR` and one of the package's
    // own files, as a `-sys` crate reports to the build scripts of dependents.
    let include = Path::new(&env::var("OUT_DIR").unwrap()).join("include");
    fs::create_dir_all(&include).unwrap();
    fs::write(include.join("answer.h"), "42").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::metadata=include={}", include.display());
    println!("cargo::metadata=data-dir={manifest_dir}/data");
}
