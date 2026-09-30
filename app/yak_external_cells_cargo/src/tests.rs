/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use serde_json::json;

use crate::CargoPlatform;
use crate::cfg::TargetCfg;
use crate::generate_third_party;
use crate::metadata::Checksums;
use crate::metadata::Metadata;

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

fn package(name: &str, version: &str, targets: serde_json::Value) -> serde_json::Value {
    json!({
        "id": format!("{REGISTRY}#{name}@{version}"),
        "name": name,
        "version": version,
        "source": REGISTRY,
        "manifest_path": format!("/registry/{name}-{version}/Cargo.toml"),
        "targets": targets,
        "authors": ["A <a@example.com>", "B"],
        "description": "A \"quoted\" description",
        "homepage": null,
        "repository": null,
        "license": "MIT",
        "links": null,
    })
}

fn lib(name: &str, version: &str, kind: &str) -> serde_json::Value {
    json!({
        "name": name,
        "kind": [kind],
        "src_path": format!("/registry/{name}-{version}/src/lib.rs"),
        "edition": "2021",
    })
}

fn build_script(name: &str, version: &str) -> serde_json::Value {
    json!({
        "name": "build-script-build",
        "kind": ["custom-build"],
        "src_path": format!("/registry/{name}-{version}/build.rs"),
        "edition": "2021",
    })
}

fn id(name: &str, version: &str) -> String {
    format!("{REGISTRY}#{name}@{version}")
}

/// A workspace member `app` that depends on `serde` and `memchr` 2, where `serde` depends on
/// `serde_derive` under the name `derive_impl`, on `libc` on Unix only, and on `memchr` 1.
fn metadata_json() -> String {
    let app = json!({
        "id": "path+file:///ws/app#0.1.0",
        "name": "app",
        "version": "0.1.0",
        "source": null,
        "manifest_path": "/ws/app/Cargo.toml",
        "targets": [{"name": "app", "kind": ["bin"], "src_path": "/ws/app/src/main.rs", "edition": "2021"}],
    });
    let packages = json!([
        app,
        package(
            "serde",
            "1.0.1-beta.2",
            json!([
                lib("serde", "1.0.1-beta.2", "lib"),
                build_script("serde", "1.0.1-beta.2")
            ])
        ),
        package(
            "serde_derive",
            "1.0.0",
            json!([lib("serde_derive", "1.0.0", "proc-macro")])
        ),
        package("libc", "0.2.0", json!([lib("libc", "0.2.0", "lib")])),
        package("memchr", "1.0.0", json!([lib("memchr", "1.0.0", "lib")])),
        package("memchr", "2.0.0", json!([lib("memchr", "2.0.0", "lib")])),
    ]);
    let normal = json!([{"kind": null, "target": null}]);
    let nodes = json!([
        {"id": "path+file:///ws/app#0.1.0", "features": [], "deps": [
            {"name": "serde", "pkg": id("serde", "1.0.1-beta.2"), "dep_kinds": normal},
            {"name": "memchr", "pkg": id("memchr", "2.0.0"), "dep_kinds": normal},
        ]},
        {"id": id("serde", "1.0.1-beta.2"), "features": ["default", "std"], "deps": [
            {"name": "derive_impl", "pkg": id("serde_derive", "1.0.0"), "dep_kinds": normal},
            {"name": "libc", "pkg": id("libc", "0.2.0"), "dep_kinds": [{"kind": null, "target": "cfg(unix)"}]},
            {"name": "memchr", "pkg": id("memchr", "1.0.0"), "dep_kinds": [{"kind": "build", "target": null}]},
        ]},
        {"id": id("serde_derive", "1.0.0"), "features": [], "deps": []},
        {"id": id("libc", "0.2.0"), "features": [], "deps": []},
        {"id": id("memchr", "1.0.0"), "features": [], "deps": []},
        {"id": id("memchr", "2.0.0"), "features": [], "deps": []},
    ]);
    json!({
        "packages": packages,
        "workspace_members": ["path+file:///ws/app#0.1.0"],
        "resolve": {"nodes": nodes},
    })
    .to_string()
}

fn metadata() -> Metadata {
    Metadata::parse(&metadata_json()).unwrap()
}

const CARGO_LOCK: &str = r#"
version = 4

[[package]]
name = "app"
version = "0.1.0"

[[package]]
name = "libc"
version = "0.2.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "1111"

[[package]]
name = "memchr"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "2222"

[[package]]
name = "memchr"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3333"

[[package]]
name = "serde"
version = "1.0.1-beta.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "4444"

[[package]]
name = "serde_derive"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "5555"
"#;

fn platforms() -> Vec<CargoPlatform> {
    vec![
        CargoPlatform {
            name: "linux-x86_64".to_owned(),
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            cfg: TargetCfg::parse("unix\ntarget_os=\"linux\"").unwrap(),
        },
        CargoPlatform {
            name: "windows-msvc".to_owned(),
            triple: "x86_64-pc-windows-msvc".to_owned(),
            cfg: TargetCfg::parse("windows\ntarget_os=\"windows\"").unwrap(),
        },
    ]
}

fn generate() -> String {
    generate_third_party(
        &metadata(),
        &Checksums::parse(CARGO_LOCK).unwrap(),
        &platforms(),
    )
    .unwrap()
}

/// The call that starts with `head` in `out`, up to the blank line that ends it.
fn call_starting<'a>(out: &'a str, head: &str) -> &'a str {
    let start = out
        .find(head)
        .unwrap_or_else(|| panic!("no `{head}` in:\n{out}"));
    let end = out[start..].find("\n\n").unwrap() + start;
    &out[start..end]
}

#[test]
fn test_archive_is_checked_by_the_lock_file_checksum() {
    let out = generate();
    assert_eq!(
        call_starting(&out, "http_archive(\n    name = \"libc-0.2.0.crate\""),
        "http_archive(\n    name = \"libc-0.2.0.crate\",\n    urls = [\"https://static.crates.io/crates/libc/libc-0.2.0.crate\"],\n    sha256 = \"1111\",\n    strip_prefix = \"libc-0.2.0\",\n    type = \"tar.gz\",\n)"
    );
}

#[test]
fn test_library_deps_features_and_platforms() {
    let out = generate();
    let serde = call_starting(
        &out,
        "cargo.rust_library(\n    name = \"serde-1.0.1-beta.2\"",
    );
    assert!(serde.contains("crate_root = \"serde-1.0.1-beta.2.crate/src/lib.rs\""));
    assert!(serde.contains("features = [\"default\", \"std\"]"));
    // The renamed proc macro applies everywhere; libc only on the Unix platform.
    assert!(serde.contains("deps = []"));
    assert!(serde.contains("named_deps = {\"derive_impl\": \":serde_derive-1.0.0\"}"));
    assert!(serde.contains("platform = {\"linux-x86_64\": {\"deps\": [\":libc-0.2.0\"]}}"));
    // The build script's results reach the library.
    assert!(
        serde
            .contains("\"OUT_DIR\": \"$(location :serde-1.0.1-beta.2-build-script-run[out_dir])\"")
    );
    assert!(serde.contains(
        "rustc_flags = [\"@$(location :serde-1.0.1-beta.2-build-script-run[rustc_flags])\"]"
    ));
    assert!(serde.contains("\"CARGO_PKG_VERSION_PATCH\": \"1\""));
    assert!(serde.contains("\"CARGO_PKG_VERSION_PRE\": \"beta.2\""));
    assert!(serde.contains("\"CARGO_PKG_AUTHORS\": \"A <a@example.com>:B\""));
    assert!(serde.contains("\"CARGO_PKG_DESCRIPTION\": \"A \\\"quoted\\\" description\""));
    assert!(!serde.contains("proc_macro"));

    let derive = call_starting(
        &out,
        "cargo.rust_library(\n    name = \"serde_derive-1.0.0\"",
    );
    assert!(derive.contains("proc_macro = True"));
}

#[test]
fn test_build_script_takes_build_deps() {
    let out = generate();
    let script = call_starting(
        &out,
        "cargo.rust_binary(\n    name = \"serde-1.0.1-beta.2-build-script-build\"",
    );
    assert!(script.contains("crate_root = \"serde-1.0.1-beta.2.crate/build.rs\""));
    assert!(script.contains("deps = [\":memchr-1.0.0\"]"));
    let run = call_starting(
        &out,
        "buildscript_run(\n    name = \"serde-1.0.1-beta.2-build-script-run\"",
    );
    assert!(run.contains("buildscript_rule = \":serde-1.0.1-beta.2-build-script-build\""));
    assert!(run.contains("manifest_dir = \":serde-1.0.1-beta.2.crate\""));
}

#[test]
fn test_alias_only_for_a_single_version() {
    let out = generate();
    assert!(out.contains("alias(\n    name = \"libc\",\n    actual = \":libc-0.2.0\","));
    assert!(!out.contains("alias(\n    name = \"memchr\""));
    // Workspace members are not third-party packages.
    assert!(!out.contains("\"app"));
}

#[test]
fn test_unsupported_source_fails() {
    let mut metadata = metadata();
    metadata.packages[2].source = Some("git+https://example.com/serde_derive".to_owned());
    let err = generate_third_party(
        &metadata,
        &Checksums::parse(CARGO_LOCK).unwrap(),
        &platforms(),
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("`serde_derive-1.0.0` comes from `git+https://example.com/serde_derive`"),
        "{err}"
    );
}

/// Adds a member `util` with a library, a build script, and an integration test, which `app`
/// depends on, and a bin of `app` named like the package.
fn workspace_metadata() -> Metadata {
    let mut value: serde_json::Value = serde_json::from_str(&metadata_json()).unwrap();
    let util_id = "path+file:///ws/util#0.2.0";
    value["packages"].as_array_mut().unwrap().push(json!({
        "id": util_id,
        "name": "util",
        "version": "0.2.0",
        "source": null,
        "manifest_path": "/ws/util/Cargo.toml",
        "targets": [
            {"name": "util", "kind": ["lib"], "src_path": "/ws/util/src/lib.rs", "edition": "2024"},
            {"name": "build-script-build", "kind": ["custom-build"], "src_path": "/ws/util/build.rs", "edition": "2024"},
            {"name": "smoke", "kind": ["test"], "src_path": "/ws/util/tests/smoke.rs", "edition": "2024"},
        ],
    }));
    value["packages"][0]["targets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "app", "kind": ["lib"], "src_path": "/ws/app/src/lib.rs", "edition": "2021"}));
    value["workspace_members"]
        .as_array_mut()
        .unwrap()
        .push(json!(util_id));
    value["resolve"]["nodes"][0]["deps"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"name": "util", "pkg": util_id, "dep_kinds": [{"kind": null, "target": null}]}),
        );
    value["resolve"]["nodes"].as_array_mut().unwrap().push(json!({
        "id": util_id,
        "features": ["fast"],
        "deps": [
            {"name": "libc", "pkg": id("libc", "0.2.0"), "dep_kinds": [{"kind": "dev", "target": "cfg(unix)"}]},
            {"name": "memchr", "pkg": id("memchr", "2.0.0"), "dep_kinds": [{"kind": "build", "target": null}]},
        ],
    }));
    Metadata::parse(&value.to_string()).unwrap()
}

fn generate_workspace_bzl() -> String {
    crate::generate_workspace(
        &workspace_metadata(),
        &platforms(),
        std::path::Path::new("/ws"),
        "crates",
    )
    .unwrap()
}

#[test]
fn test_workspace_member_labels() {
    let out = generate_workspace_bzl();
    // `app` has a bin named like the package, so its library is `app-lib`.
    assert!(
        out.contains(
            "\"lib\": {\"rule\": \"app-lib\", \"crate\": \"app\", \"crate_root\": \"src/lib.rs\""
        ),
        "{out}"
    );
    assert!(out.contains(
        "\"bins\": [{\"rule\": \"app\", \"crate\": \"app\", \"crate_root\": \"src/main.rs\""
    ));
    // Third-party dependencies name the cargo cell, and members name their directory.
    assert!(out.contains(
        "\"deps\": [\"crates//:serde-1.0.1-beta.2\", \"crates//:memchr-2.0.0\", \"//util:util\"]"
    ));
}

#[test]
fn test_workspace_member_build_script_features_and_tests() {
    let out = generate_workspace_bzl();
    let util = &out[out.find("\"util\": {").unwrap()..];
    assert!(util.contains("\"features\": [\"fast\"]"));
    assert!(util.contains("\"build_script\": {\"rule\": \"\", \"crate\": \"build_script_build\", \"crate_root\": \"build.rs\", \"edition\": \"2024\"}"));
    assert!(util.contains("\"build_deps\": [\"crates//:memchr-2.0.0\"]"));
    assert!(util.contains(
        "\"tests\": [{\"rule\": \"smoke\", \"crate\": \"smoke\", \"crate_root\": \"tests/smoke.rs\""
    ));
    // The dev dependency applies to tests on Unix only.
    assert!(
        util.contains(
            "\"test_platform\": {\"linux-x86_64\": {\"deps\": [\"crates//:libc-0.2.0\"]}}"
        )
    );
    assert!(util.contains("\"CARGO_MANIFEST_DIR\": \".\""));
    assert!(out.contains("def cargo_workspace_member():"));
}
