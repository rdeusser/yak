// Links a library from a directory outside the package, as a build script
// that finds a system library through pkg-config does. The test writes the
// directory into `host-lib-dir.txt`.
fn main() {
    let dir = std::fs::read_to_string("host-lib-dir.txt").unwrap();
    println!("cargo:rerun-if-changed=host-lib-dir.txt");
    println!("cargo:rustc-link-search=native={}", dir.trim());
    println!("cargo:rustc-link-lib=hostanswer");
}
