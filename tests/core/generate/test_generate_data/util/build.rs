use std::env;
use std::fs;
use std::path::Path;

// Cargo sets these for every build script, and some scripts read them
// without a fallback.
const REQUIRED: &[&str] = &[
    "CARGO_PKG_VERSION_MAJOR",
    "CARGO_PKG_VERSION_PRE",
    "DEBUG",
    "NUM_JOBS",
    "OPT_LEVEL",
    "PROFILE",
    "RUSTDOC",
];

fn main() {
    for name in REQUIRED {
        env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    }
    let out_dir = env::var("OUT_DIR").unwrap();
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    fs::write(
        Path::new(&out_dir).join("major.rs"),
        format!("pub const MAJOR: &str = {major:?};\n"),
    )
    .unwrap();
    println!("cargo:rustc-env=FROM_BUILD_SCRIPT=yes");
}
