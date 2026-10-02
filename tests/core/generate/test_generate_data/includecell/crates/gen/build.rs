use std::fs;

fn main() {
    // `assets` is a cell, and `sub` is a package of it.
    let top = fs::read_to_string("../../assets/top.txt").unwrap();
    let sub = fs::read_to_string("../../assets/sub/sub.txt").unwrap();
    println!("cargo::rerun-if-changed=../../assets");
    println!("cargo::rustc-env=ASSETS={} {}", top.trim(), sub.trim());
}
