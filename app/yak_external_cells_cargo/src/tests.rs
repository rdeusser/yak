/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use serde_json::json;

use crate::CargoPlatform;
use crate::ThirdParty;
use crate::cfg::TargetCfg;
use crate::generate_third_party;
use crate::includes::IncludedFile;
use crate::includes::IncludedPath;
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
        "workspace_root": "/ws",
        "resolve": {"nodes": nodes},
    })
    .to_string()
}

fn metadata() -> Metadata {
    Metadata::parse(&metadata_json()).unwrap()
}

fn platforms() -> Vec<CargoPlatform> {
    vec![
        CargoPlatform {
            name: "linux-x86_64".to_owned(),
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            cfg: TargetCfg::parse("unix\ntarget_os=\"linux\"").unwrap(),
            rustflags: Vec::new(),
        },
        CargoPlatform {
            name: "windows-msvc".to_owned(),
            triple: "x86_64-pc-windows-msvc".to_owned(),
            cfg: TargetCfg::parse("windows\ntarget_os=\"windows\"").unwrap(),
            rustflags: Vec::new(),
        },
    ]
}

fn generate() -> ThirdParty {
    generate_third_party(&metadata(), &platforms()).unwrap()
}

/// The build file of the package in directory `dir`.
fn build_file(third_party: &ThirdParty, dir: &str) -> String {
    third_party
        .packages
        .iter()
        .find(|p| p.dir == dir)
        .unwrap_or_else(|| panic!("no package `{dir}`"))
        .build_file
        .clone()
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
fn test_packages_come_from_the_directories_of_their_manifests() {
    let third_party = generate();
    let dirs: Vec<(&str, &str)> = third_party
        .packages
        .iter()
        .map(|p| (p.dir.as_str(), p.source_dir.as_str()))
        .collect();
    assert_eq!(
        dirs,
        [
            ("libc-0.2.0", "/registry/libc-0.2.0"),
            ("memchr-1.0.0", "/registry/memchr-1.0.0"),
            ("memchr-2.0.0", "/registry/memchr-2.0.0"),
            ("serde-1.0.1-beta.2", "/registry/serde-1.0.1-beta.2"),
            ("serde_derive-1.0.0", "/registry/serde_derive-1.0.0"),
        ]
    );
}

#[test]
fn test_library_deps_features_and_platforms() {
    let third_party = generate();
    let out = build_file(&third_party, "serde-1.0.1-beta.2");
    let serde = call_starting(&out, "cargo.rust_library(\n    name = \"serde\"");
    assert!(serde.contains("crate_root = \"src/lib.rs\""));
    assert!(serde.contains("srcs = glob([\"**\", \"**/.*\", \"**/.*/**\"], exclude = [\"YAK\"])"));
    assert!(serde.contains("features = [\"default\", \"std\"]"));
    // The renamed proc macro applies everywhere; libc only on the Unix platform. The run of the
    // build script carries the search paths of host libraries to dependents' links.
    assert!(serde.contains("deps = [\":serde-build-script-run\"]"));
    assert!(
        serde.contains("named_deps = {\"derive_impl\": \"//serde_derive-1.0.0:serde_derive\"}")
    );
    assert!(serde.contains("platform = {\"linux-x86_64\": {\"deps\": [\"//libc-0.2.0:libc\"]}}"));
    // The build script's results reach the library.
    assert!(serde.contains("\"OUT_DIR\": \"$(location :serde-build-script-run[out_dir])\""));
    assert!(
        serde.contains("rustc_flags = [\"@$(location :serde-build-script-run[rustc_flags])\"]")
    );
    assert!(serde.contains("\"CARGO_MANIFEST_DIR\": \".\""));
    assert!(serde.contains("\"CARGO_PKG_VERSION_PATCH\": \"1\""));
    assert!(serde.contains("\"CARGO_PKG_VERSION_PRE\": \"beta.2\""));
    assert!(serde.contains("\"CARGO_PKG_AUTHORS\": \"A <a@example.com>:B\""));
    assert!(serde.contains("\"CARGO_PKG_DESCRIPTION\": \"A \\\"quoted\\\" description\""));
    assert!(!serde.contains("proc_macro"));

    let derive = build_file(&third_party, "serde_derive-1.0.0");
    assert!(call_starting(&derive, "cargo.rust_library(").contains("proc_macro = True"));
    assert!(!derive.contains("buildscript_run"));
}

#[test]
fn test_build_script_takes_build_deps() {
    let third_party = generate();
    let out = build_file(&third_party, "serde-1.0.1-beta.2");
    let script = call_starting(
        &out,
        "cargo.rust_binary(\n    name = \"serde-build-script-build\"",
    );
    assert!(script.contains("crate_root = \"build.rs\""));
    assert!(script.contains("deps = [\"//memchr-1.0.0:memchr\"]"));
    let run = call_starting(
        &out,
        "buildscript_run(\n    name = \"serde-build-script-run\"",
    );
    assert!(run.contains("buildscript_rule = \":serde-build-script-build\""));
    assert!(run.contains("manifest_dir = \":serde-manifest-dir\""));
    assert!(run.contains("rustc_link_lib = True,\n    rustc_link_search = True,"));
}

#[test]
fn test_build_script_takes_the_runs_of_normal_deps_with_links() {
    // `libc` (a normal dependency of `serde` on Unix) and `memchr` 1 (a build dependency) set
    // `links` and have build scripts. Cargo passes the metadata of `libc` alone to the build
    // script of `serde`.
    let mut json: serde_json::Value = serde_json::from_str(&metadata_json()).unwrap();
    for package in json["packages"].as_array_mut().unwrap() {
        let (name, version) = match (package["name"].as_str(), package["version"].as_str()) {
            (Some(name @ ("libc" | "memchr")), Some(version @ ("0.2.0" | "1.0.0"))) => {
                (name.to_owned(), version.to_owned())
            }
            _ => continue,
        };
        package["links"] = json!(name);
        package["targets"]
            .as_array_mut()
            .unwrap()
            .push(build_script(&name, &version));
    }
    let metadata = Metadata::parse(&json.to_string()).unwrap();
    let third_party = generate_third_party(&metadata, &platforms()).unwrap();
    let out = build_file(&third_party, "serde-1.0.1-beta.2");
    let run = call_starting(
        &out,
        "buildscript_run(\n    name = \"serde-build-script-run\"",
    );
    assert!(run.contains("links_deps = []"), "{run}");
    assert!(
        run.contains(
            "platform = {\"linux-x86_64\": {\"links_deps\": [\"//libc-0.2.0:libc-build-script-run\"]}}"
        ),
        "{run}"
    );
    assert!(!run.contains("memchr"), "{run}");
    // The run of the build script of `libc` writes metadata when it gets `CARGO_MANIFEST_LINKS`.
    let libc = build_file(&third_party, "libc-0.2.0");
    let libc_run = call_starting(
        &libc,
        "buildscript_run(\n    name = \"libc-build-script-run\"",
    );
    assert!(libc_run.contains("\"CARGO_MANIFEST_LINKS\": \"libc\""));
    assert!(libc_run.contains("visibility = [\"PUBLIC\"]"));
}

/// The platforms of `platforms()` with the `rustflags` of each.
fn platforms_with_rustflags(linux: &[&str], windows: &[&str]) -> Vec<CargoPlatform> {
    let mut platforms = platforms();
    platforms[0].rustflags = linux.iter().map(|f| (*f).to_owned()).collect();
    platforms[1].rustflags = windows.iter().map(|f| (*f).to_owned()).collect();
    platforms
}

#[test]
fn test_rustflags_of_every_platform_apply_to_every_crate() {
    let flags = ["--cfg", "tokio_unstable"];
    let third_party =
        generate_third_party(&metadata(), &platforms_with_rustflags(&flags, &flags)).unwrap();
    let out = build_file(&third_party, "serde-1.0.1-beta.2");
    let serde = call_starting(&out, "cargo.rust_library(\n    name = \"serde\"");
    assert!(serde.contains(
        "rustc_flags = [\"--cfg\", \"tokio_unstable\", \"@$(location :serde-build-script-run[rustc_flags])\"]"
    ));
    let script = call_starting(
        &out,
        "cargo.rust_binary(\n    name = \"serde-build-script-build\"",
    );
    assert!(script.contains("rustc_flags = [\"--cfg\", \"tokio_unstable\"]"));

    let workspace = crate::generate_workspace(
        &workspace_metadata(),
        &platforms_with_rustflags(&flags, &flags),
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &util_includes(),
    )
    .unwrap();
    assert!(workspace.contains("\n_RUSTFLAGS = [\"--cfg\", \"tokio_unstable\"]\n"));
    assert!(workspace.contains("rustc_flags = list(_RUSTFLAGS)"));
    assert!(workspace.contains("rustc_flags = _RUSTFLAGS,"));
}

#[test]
fn test_rustflags_that_differ_by_platform_select_on_it() {
    let platforms = platforms_with_rustflags(&["-Ctarget-cpu=native"], &[]);
    let third_party = generate_third_party(&metadata(), &platforms).unwrap();
    let out = build_file(&third_party, "memchr-2.0.0");
    let memchr = call_starting(&out, "cargo.rust_library(");
    assert!(memchr.contains("rustc_flags = []"), "{memchr}");
    assert!(
        memchr.contains(
            "platform = {\"linux-x86_64\": {\"rustc_flags\": [\"-Ctarget-cpu=native\"]}}"
        ),
        "{memchr}"
    );

    let workspace = crate::generate_workspace(
        &workspace_metadata(),
        &platforms,
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &util_includes(),
    )
    .unwrap();
    assert!(workspace.contains("\n_RUSTFLAGS = []\n"));
    assert!(member(&workspace, "util").contains(
        "\"test_platform\": {\"linux-x86_64\": {\"deps\": [\"crates//libc-0.2.0:libc\"], \"rustc_flags\": [\"-Ctarget-cpu=native\"]}}"
    ));
}

#[test]
fn test_aliases() {
    let out = generate().root_build_file;
    assert!(
        out.contains("alias(\n    name = \"libc-0.2.0\",\n    actual = \"//libc-0.2.0:libc\",")
    );
    assert!(out.contains("alias(\n    name = \"libc\",\n    actual = \"//libc-0.2.0:libc\","));
    assert!(out.contains("alias(\n    name = \"memchr-2.0.0\""));
    assert!(!out.contains("alias(\n    name = \"memchr\""));
    // Workspace members are not third-party packages.
    assert!(!out.contains("\"app"));
}

#[test]
fn test_package_from_another_source_is_named_by_its_id() {
    let mut metadata = metadata();
    metadata.packages[2].source = Some("git+https://example.com/serde_derive#0123".to_owned());
    let dir = crate::third_party::cell_dir(&metadata.packages[2]);
    assert!(dir.starts_with("serde_derive-1.0.0-") && dir.len() == "serde_derive-1.0.0-".len() + 8);

    // Another commit gets another directory, so the cell never reuses the old copy.
    let mut other = metadata.packages[2].clone();
    other.source = Some("git+https://example.com/serde_derive#4567".to_owned());
    assert_ne!(crate::third_party::cell_dir(&other), dir);

    let third_party = generate_third_party(&metadata, &platforms()).unwrap();
    let serde = build_file(&third_party, "serde-1.0.1-beta.2");
    assert!(serde.contains(&format!("\"derive_impl\": \"//{dir}:serde_derive\"")));
    assert!(third_party.root_build_file.contains(&format!(
        "alias(\n    name = \"serde_derive-1.0.0\",\n    actual = \"//{dir}:serde_derive\","
    )));
}

#[test]
fn test_path_dependency_outside_the_workspace_fails() {
    let mut metadata = metadata();
    metadata.packages[2].source = None;
    let err = generate_third_party(&metadata, &platforms()).unwrap_err();
    assert!(
        err.to_string()
            .contains("`serde_derive-1.0.0` is a path dependency outside the workspace"),
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
        "metadata": null,
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

/// `workspace.bzl` for a workspace in the directory `ws` of the project.
fn generate_workspace_bzl() -> String {
    crate::generate_workspace(
        &workspace_metadata(),
        &platforms(),
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &util_includes(),
    )
    .unwrap()
}

/// The data of the member in `dir`.
fn member<'a>(out: &'a str, dir: &str) -> &'a str {
    let start = out
        .find(&format!("\"dir\": \"{dir}\""))
        .unwrap_or_else(|| panic!("no member `{dir}` in:\n{out}"));
    // The next member's version follows its name and precedes its directory.
    let end = out[start..]
        .find(", \"version\": ")
        .map_or(out.len(), |i| start + i);
    &out[start..end]
}

#[test]
fn test_workspace_member_labels() {
    let out = generate_workspace_bzl();
    assert!(out.contains("_WORKSPACE_DIR = \"ws\""));
    // Each member's data is keyed by its package.
    assert!(
        out.contains("_MEMBERS = {\"ws/app\": {\"name\": \"app\""),
        "{out}"
    );
    let app = member(&out, "app");
    // `app` has a bin named like the package, so its library is `app-lib`.
    assert!(
        app.contains(
            "\"lib\": {\"rule\": \"app-lib\", \"crate\": \"app\", \"crate_root\": \"app/src/lib.rs\""
        ),
        "{app}"
    );
    assert!(app.contains(
        "\"bins\": [{\"rule\": \"app\", \"crate\": \"app\", \"crate_root\": \"app/src/main.rs\""
    ));
    // Third-party dependencies name the cargo cell, and members name the package of their
    // directory.
    assert!(app.contains(
        "\"deps\": [\"crates//serde-1.0.1-beta.2:serde\", \"crates//memchr-2.0.0:memchr\", \"//ws/util:util\"]"
    ));
    assert!(app.contains("\"srcs_exclude\": [\"target/**\", \"YAK\"]"));
    assert!(app.contains("\"CARGO_MANIFEST_DIR\": \"app\""));
    // `//ws/app` names the binary, which keeps the package's name.
    assert!(app.contains("\"alias\": None"), "{app}");
}

#[test]
fn test_workspace_member_build_script_features_and_tests() {
    let out = generate_workspace_bzl();
    let util = member(&out, "util");
    assert!(util.contains("\"features\": [\"fast\"]"));
    assert!(util.contains("\"build_script\": {\"rule\": \"\", \"crate\": \"build_script_build\", \"crate_root\": \"util/build.rs\", \"edition\": \"2024\"}"));
    assert!(util.contains("\"build_deps\": [\"crates//memchr-2.0.0:memchr\"]"));
    assert!(out.contains("script_deps = [\":\" + names[\"build_script_run\"]]"));
    assert!(util.contains(
        "\"tests\": [{\"rule\": \"util-smoke\", \"crate\": \"smoke\", \"crate_root\": \"util/tests/smoke.rs\""
    ));
    assert!(util.contains("\"unittest\": \"util-unittest\""));
    // The library and binaries list the member's tests, so `yak test //ws/util` runs them.
    assert!(util.contains("\"test_rules\": [\"util-unittest\", \"util-smoke\"]"));
    assert!(util.contains("\"build_script_run\": \"util-build-script-run\""));
    // The dev dependency applies to tests on Unix only.
    assert!(util.contains(
        "\"test_platform\": {\"linux-x86_64\": {\"deps\": [\"crates//libc-0.2.0:libc\"]}}"
    ));
    assert!(out.contains("def cargo_package(include = [], test_data = []):"));
    assert!(out.contains("def cargo_workspace():"));
}

#[test]
fn test_workspace_member_alias_names_the_library_by_directory() {
    let mut metadata = workspace_metadata();
    let util = metadata
        .packages
        .iter_mut()
        .find(|p| p.name == "util")
        .unwrap();
    util.manifest_path = "/ws/crates/utility/Cargo.toml".to_owned();
    for target in &mut util.targets {
        target.src_path = target.src_path.replace("/ws/util/", "/ws/crates/utility/");
    }
    let out = crate::generate_workspace(
        &metadata,
        &platforms(),
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(
        member(&out, "crates/utility")
            .contains("\"alias\": {\"name\": \"utility\", \"actual\": \"util\"}"),
        "{out}"
    );
    assert!(member(&out, "app").contains("\"//ws/crates/utility:util\""));
}

/// The includes of a workspace whose `util/src/lib.rs` includes a file of the workspace's root,
/// a file of a hand-written package, a file of `app`, and a file of its own.
fn util_includes() -> BTreeMap<String, Vec<IncludedFile>> {
    let file = |path: &str, owner: &str| IncludedFile {
        path: path.to_owned(),
        owner: owner.to_owned(),
    };
    BTreeMap::from([(
        "util".to_owned(),
        vec![
            file("shared/banner.txt", ""),
            file("assets/logo.png", "assets"),
            file("app/data.txt", "app"),
            file("util/own.txt", "util"),
        ],
    )])
}

#[test]
fn test_workspace_member_includes_files_of_other_packages() {
    let out = generate_workspace_bzl();
    // A file of the member's own package is one of its sources already.
    assert!(
        member(&out, "util").contains(
            "\"include\": {\"//ws:shared/banner.txt\": \"\", \"//ws/assets:logo.png\": \"assets\", \"//ws/app:data.txt\": \"app\"}"
        ),
        "{out}"
    );
    // The packages that the cell declares export the files to `util`. The hand-written build file
    // of `assets` declares its own target.
    assert!(
        out.contains(
            "_EXPORTS = {\"ws\": {\"shared/banner.txt\": [\"ws/util\"]}, \"ws/app\": {\"data.txt\": [\"ws/util\"]}}"
        ),
        "{out}"
    );

    let mut metadata = workspace_metadata();
    let util = metadata
        .packages
        .iter_mut()
        .find(|p| p.name == "util")
        .unwrap();
    util.metadata = Some(json!({"yak": {"test-data": ["../shared.txt"]}}));
    let err = crate::generate_workspace(
        &metadata,
        &platforms(),
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("`[package.metadata.yak]` of package `util` is no longer read"),
        "{err}"
    );
}

#[test]
fn test_resolve_includes() {
    let includes = [
        (
            "util/src/lib.rs".to_owned(),
            IncludedPath::RelativeToSource("../../shared/banner.txt".to_owned()),
        ),
        (
            "util/src/lib.rs".to_owned(),
            IncludedPath::RelativeToSource("own.txt".to_owned()),
        ),
        (
            "util/src/lib.rs".to_owned(),
            IncludedPath::RelativeToManifestDir("/../shared/manifest.txt".to_owned()),
        ),
        // A path outside the workspace belongs to no member.
        (
            "util/src/lib.rs".to_owned(),
            IncludedPath::RelativeToSource("../../../outside.txt".to_owned()),
        ),
    ];
    assert_eq!(
        crate::resolve_includes("util", &includes),
        [
            "shared/banner.txt",
            "shared/manifest.txt",
            "util/src/own.txt"
        ]
    );
}

#[test]
fn test_workspace_member_at_the_root() {
    let mut metadata = workspace_metadata();
    let app = metadata
        .packages
        .iter_mut()
        .find(|p| p.name == "app")
        .unwrap();
    app.manifest_path = "/ws/Cargo.toml".to_owned();
    for target in &mut app.targets {
        target.src_path = target.src_path.replace("/ws/app/", "/ws/");
    }
    let out = crate::generate_workspace(
        &metadata,
        &platforms(),
        std::path::Path::new("/ws"),
        "crates",
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(out.contains("_WORKSPACE_DIR = \"\""));
    assert!(
        out.contains("_MEMBERS = {\"\": {\"name\": \"app\""),
        "{out}"
    );
    let root = member(&out, "");
    // Members below the root are packages of their own, which a glob leaves out.
    assert!(
        root.contains("\"srcs_exclude\": [\".git/**\", \"target/**\", \"yak-out/**\", \"YAK\"]")
    );
    assert!(root.contains("\"CARGO_MANIFEST_DIR\": \".\""));
    assert!(root.contains("\"//util:util\""));
}

#[test]
fn test_workspace_duplicate_target_fails() {
    let mut metadata = workspace_metadata();
    // An integration test named `unittest` gets the name of the library's unit tests.
    let util = metadata
        .packages
        .iter_mut()
        .find(|p| p.name == "util")
        .unwrap();
    util.targets[2].name = "unittest".to_owned();
    let err = crate::generate_workspace(
        &metadata,
        &platforms(),
        std::path::Path::new("/"),
        "crates",
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Two targets of the Cargo package `util` are named `util-unittest`"),
        "{err}"
    );
}

#[test]
fn test_workspace_layout_member_dirs() {
    let layout = crate::metadata::WorkspaceLayout::parse(
        &json!({
            "packages": [
                {"id": "path+file:///ws#root@0.1.0", "name": "root", "version": "0.1.0", "source": null,
                 "manifest_path": "/ws/Cargo.toml", "targets": []},
                {"id": "path+file:///ws/crates/b#0.1.0", "name": "b", "version": "0.1.0", "source": null,
                 "manifest_path": "/ws/crates/b/Cargo.toml", "targets": []},
                {"id": "path+file:///ws/crates/a#0.1.0", "name": "a", "version": "0.1.0", "source": null,
                 "manifest_path": "/ws/crates/a/Cargo.toml", "targets": []},
                {"id": "path+file:///elsewhere#0.1.0", "name": "elsewhere", "version": "0.1.0", "source": null,
                 "manifest_path": "/elsewhere/Cargo.toml", "targets": []},
            ],
            "workspace_members": [
                "path+file:///ws#root@0.1.0",
                "path+file:///ws/crates/b#0.1.0",
                "path+file:///ws/crates/a#0.1.0",
            ],
            "workspace_root": "/ws",
            "resolve": null,
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(layout.member_dirs().unwrap(), ["", "crates/a", "crates/b"]);
}
