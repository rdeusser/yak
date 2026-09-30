// Compiles a shared library into `OUT_DIR` and links it, so that the binaries
// that link `seven` load `libseven` at run time.
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let lib_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("lib");
    std::fs::create_dir_all(&lib_dir).unwrap();
    let macos = env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos";
    let name = if macos { "libseven.dylib" } else { "libseven.so" };
    let mut cc = Command::new(env::var("CC").unwrap_or_else(|_| "cc".to_owned()));
    cc.args(["-shared", "-fPIC", "seven.c", "-o"]).arg(lib_dir.join(name));
    if macos {
        cc.arg("-Wl,-install_name,@rpath/libseven.dylib");
    }
    assert!(cc.status().unwrap().success());
    println!("cargo:rerun-if-changed=seven.c");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=seven");
}
