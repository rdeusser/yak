/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Writes `workspace.bzl`, which holds the data of the workspace members and the
//! `cargo_workspace` macro that declares their targets from it. The build file at the root of
//! the workspace only calls the macro, so all members are targets of one package, and a crate
//! can read files of the workspace outside its own directory.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use yak_external_cells_starlark::Value;

use crate::graph::CargoPlatform;
use crate::graph::Deps;
use crate::graph::GenerateError;
use crate::graph::Graph;
use crate::graph::PlatformFlags;
use crate::graph::crate_name;
use crate::graph::lib_target;
use crate::graph::package_env;
use crate::graph::relative_to;
use crate::includes::IncludedFile;
use crate::includes::IncludedPath;
use crate::metadata::Metadata;
use crate::metadata::Package;
use crate::metadata::Target;
use crate::profile::PANIC_ABORT_TRANSITION;
use crate::profile::Profile;
use crate::profile::panic_flags_select;
use crate::third_party::library_label;
use crate::third_party::script_run_label;

/// The macros that the build files of the workspace call. `_WORKSPACE_DIR` is the directory of
/// the workspace relative to the project root, `_CELL_DIRS` holds the directory of each cell
/// relative to the project root by the cell's name, `_MEMBERS` holds the data of each member by its
/// package, `_EXPORTS` holds, by package, the files of the package that other members include
/// and the packages that include each, and `_RUSTFLAGS` holds the flags of Cargo's configuration
/// when every platform has the same. The `platform` data of a member holds them otherwise.
/// `_PANIC_FLAGS` selects `-Cpanic=abort` in the configurations of binaries that abort on panic,
/// and `_BINARY_ATTRS` gives the binaries their transition to such a configuration.
const MACRO: &str = r#"
def cargo_package(include = [], test_data = []):
    """Declares the targets of the member of the Cargo workspace in this directory.

    The member builds from the files of its directory and the files that its Rust sources name in
    `include!`, `include_str!`, and `include_bytes!`. `include` names the targets of other
    packages whose files its crates read at compile time, and `test_data` the targets whose files
    its tests read at run time, such as `filegroup`s. The targets can belong to other cells inside
    the workspace's directory. Their files appear at their paths in the workspace."""
    member = _MEMBERS.get(package_name())
    if member == None:
        fail("`cargo_package()` belongs in the build file of a member of the Cargo workspace at `{}`, and `{}` is not one".format(_WORKSPACE_DIR or ".", package_name() or "."))
    _declare_exports()
    _declare_member(member, _package_dirs(include, "include"), _package_dirs(test_data, "test_data"))

def cargo_workspace():
    """Declares the files of the root of the Cargo workspace in this directory that its members
    include, when the root is no member."""
    if package_name() != _WORKSPACE_DIR:
        fail("`cargo_workspace()` belongs in the build file of `{}`, the root of the Cargo workspace".format(_WORKSPACE_DIR or "."))
    if package_name() in _MEMBERS:
        fail("The root of the Cargo workspace is a member, so its build file calls `cargo_package()`")
    _declare_exports()

def _declare_exports():
    for path, users in _EXPORTS.get(package_name(), {}).items():
        native.export_file(
            name = path,
            src = path,
            visibility = ["//{}:".format(user) for user in users],
        )

def _package_dirs(labels, arg):
    """The directory of each target's package relative to the workspace's directory, by label."""
    prefix = _WORKSPACE_DIR + "/" if _WORKSPACE_DIR else ""
    dirs = {}
    for label in labels:
        if label.startswith(":"):
            cell, package = get_cell_name(), package_name()
        elif "//" in label:
            cell, rest = label.removeprefix("@").split("//", 1)
            cell = cell or get_cell_name()
            package = rest.split(":")[0]
        else:
            fail("`{}` takes labels of targets, such as `//crates/data:testdata`, and `{}` is not one".format(arg, label))
        if cell not in _CELL_DIRS:
            fail("`{}` names `{}`, and `{}` is no cell of the project".format(arg, label, cell))
        path = "/".join([p for p in [_CELL_DIRS[cell], package] if p])
        if path == _WORKSPACE_DIR:
            dirs[label] = ""
        elif path.startswith(prefix):
            dirs[label] = path.removeprefix(prefix)
        else:
            fail("`{}` names `{}`, whose package is outside the Cargo workspace at `{}`".format(arg, label, _WORKSPACE_DIR or "."))
    return dirs

def _declare(rule, platform, **kwargs):
    # The sources sit at their paths in the workspace, so `file!()` and panic locations name
    # them relative to the workspace's directory, as with Cargo.
    rule(srcs_path = _WORKSPACE_DIR, **apply_platform_attrs(platform, kwargs))

# The file names of Cargo's outputs on each platform, formatted with the target's name.
_MACOS_FILES = struct(exe = "{}", dylib = "lib{}.dylib", staticlib = "lib{}.a")
_LINUX_FILES = struct(exe = "{}", dylib = "lib{}.so", staticlib = "lib{}.a")
_WINDOWS_FILES = struct(exe = "{}.exe", dylib = "{}.dll", staticlib = "{}.lib")

# The sub-target of a `rust_library` that builds each crate type of a library example.
_EXAMPLE_SUB_TARGETS = {
    "cdylib": ("[cdylib]", "dylib"),
    "dylib": ("[dylib]", "dylib"),
    "staticlib": ("[staticlib]", "staticlib"),
}

def _profile_files(member, files):
    """The member's binaries and examples at their paths in Cargo's profile directory, where
    `cargo test` builds them and a test can find them from its own path."""
    out = {}
    for target in member["bins"]:
        out[files.exe.format(target["name"])] = ":" + target["rule"]
    for example in member["examples"]:
        if example["crate_types"] == ["bin"]:
            out["examples/" + files.exe.format(example["name"])] = ":" + example["rule"]
            continue
        stem = example["name"].replace("-", "_")
        for crate_type in example["crate_types"]:
            if crate_type in _EXAMPLE_SUB_TARGETS:
                sub_target, kind = _EXAMPLE_SUB_TARGETS[crate_type]
                name = getattr(files, kind).format(stem)
                out["examples/" + name] = ":" + example["rule"] + sub_target
    return out

def _declare_member(member, include, test_data):
    srcs = glob(["**", "**/.*", "**/.*/**"], exclude = member["srcs_exclude"])

    # A crate's sources sit at their paths in the workspace, so that a path that leaves the
    # member's directory, such as `include_str!("../../README.md")`, resolves as with Cargo.
    prefix = member["dir"] + "/" if member["dir"] else ""
    mapped_srcs = {f: prefix + f for f in srcs}
    package_srcs = dict(member["include"])
    package_srcs.update(include)

    # A test reads its member's files, its included files, and its test data from a copy of them,
    # so a read of another file fails.
    test_package_srcs = dict(package_srcs)
    test_package_srcs.update(test_data)

    env = dict(member["env"])

    # Cargo passes the flags of the profile before those of its configuration, so `rustflags` can
    # override them. Build scripts and procedural macros compile with the host settings.
    profile = member["profile"]
    rustc_flags = profile["flags"] + _RUSTFLAGS
    host_rustc_flags = profile["host_flags"] + _RUSTFLAGS
    names = member["rules"]

    # The run target of the build script carries the search paths of the host libraries that the
    # script links to every link of a dependent.
    script_deps = []

    script = member["build_script"]
    if script != None:
        script_env = dict(env)
        script_env.update(profile["script_env"])

        # A build script builds for the execution platform of its run target. The requirement
        # keeps `//...` from building the script and its per-platform aliases for the target
        # platform and for every Cargo platform.
        exec_only = [get_exec_platform_marker()]

        cargo.rust_binary(
            name = names["build_script_build"],
            crate = "build_script_build",
            crate_root = script["crate_root"],
            mapped_srcs = mapped_srcs,
            srcs_path = _WORKSPACE_DIR,
            package_srcs = package_srcs,
            edition = script["edition"],
            features = member["features"],
            deps = member["build_deps"],
            named_deps = member["build_named_deps"],
            platform = member["build_platform"],
            env = env,
            rustc_flags = host_rustc_flags,
            target_compatible_with = exec_only,
            visibility = [],
        )
        buildscript_run(
            name = names["build_script_run"],
            buildscript_rule = ":" + names["build_script_build"],
            package_name = member["name"],
            version = member["version"],
            features = member["features"],
            # The build script runs in the member's directory of a tree with the crate's sources,
            # so that a path that leaves the directory, such as `../proto/api.proto`, resolves.
            filegroup_for_manifest_dir = {path: f for f, path in mapped_srcs.items()},
            manifest_subdir = member["dir"],
            package_srcs = package_srcs,
            links_deps = member["links_deps"],
            platform = member["links_platform"],
            buildscript_compatible_with = exec_only,
            # The build scripts of dependents read the metadata of a package with `links`.
            visibility = ["PUBLIC"] if "CARGO_MANIFEST_LINKS" in env else [],
            env = script_env,
            rustc_link_lib = True,
            rustc_link_search = True,
        )
        env["OUT_DIR"] = "$(location :{}[out_dir])".format(names["build_script_run"])
        script_flags = "@$(location :{}[rustc_flags])".format(names["build_script_run"])
        rustc_flags.append(script_flags)
        host_rustc_flags.append(script_flags)
        script_deps = [":" + names["build_script_run"]]

    profile_files = select({
        "DEFAULT": _profile_files(member, _LINUX_FILES),
        "prelude//os:macos": _profile_files(member, _MACOS_FILES),
        "prelude//os:windows": _profile_files(member, _WINDOWS_FILES),
    })

    # `yak test` on the library or a binary runs the tests of the member, as `cargo test` does.
    tests = [":" + rule for rule in member["test_rules"]]

    lib = member["lib"]
    own = []
    if lib != None:
        own = [":" + lib["rule"]]
        _declare(
            native.rust_library,
            member["platform"],
            name = lib["rule"],
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            mapped_srcs = mapped_srcs,
            package_srcs = package_srcs,
            edition = lib["edition"],
            proc_macro = lib["proc_macro"],
            features = member["features"],
            deps = member["deps"] + script_deps,
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = host_rustc_flags if lib["proc_macro"] else rustc_flags + _PANIC_FLAGS,
            tests = tests,
            visibility = ["PUBLIC"],
        )
    if lib != None and lib["unittest"] != None:
        _declare(
            native.rust_test,
            member["test_platform"],
            name = lib["unittest"],
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            mapped_srcs = mapped_srcs,
            package_srcs = test_package_srcs,
            edition = lib["edition"],
            features = member["features"],
            deps = member["test_deps"] + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            cargo_target_files = profile_files,
            run_from_manifest_dir = True,
            supports_test_execution_caching = True,
        )

    for target in member["bins"]:
        _declare(
            native.rust_binary,
            member["platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            mapped_srcs = mapped_srcs,
            package_srcs = package_srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["deps"] + own + script_deps,
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = rustc_flags + _PANIC_FLAGS,
            tests = tests,
            visibility = ["PUBLIC"],
            **_BINARY_ATTRS
        )
        if target["unittest"] != None:
            _declare(
                native.rust_test,
                member["test_platform"],
                name = target["unittest"],
                crate = target["crate"],
                crate_root = target["crate_root"],
                mapped_srcs = mapped_srcs,
                package_srcs = test_package_srcs,
                edition = target["edition"],
                features = member["features"],
                deps = member["test_deps"] + own + script_deps,
                named_deps = member["test_named_deps"],
                env = env,
                rustc_flags = rustc_flags,
                cargo_target_files = profile_files,
                run_from_manifest_dir = True,
                supports_test_execution_caching = True,
            )

    for example in member["examples"]:
        _declare(
            native.rust_binary if example["crate_types"] == ["bin"] else native.rust_library,
            member["test_platform"],
            name = example["rule"],
            crate = example["crate"],
            crate_root = example["crate_root"],
            mapped_srcs = mapped_srcs,
            package_srcs = package_srcs,
            edition = example["edition"],
            features = member["features"],
            deps = member["test_deps"] + own + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags + _PANIC_FLAGS,
            **(_BINARY_ATTRS if example["crate_types"] == ["bin"] else {})
        )

    # Cargo gives integration tests the paths of the package's binaries.
    test_env = dict(env)
    for target in member["bins"]:
        test_env["CARGO_BIN_EXE_" + target["name"]] = "$(location :{})".format(target["rule"])

    for target in member["tests"]:
        _declare(
            native.rust_test,
            member["test_platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            mapped_srcs = mapped_srcs,
            package_srcs = test_package_srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["test_deps"] + own + script_deps,
            named_deps = member["test_named_deps"],
            env = test_env,
            rustc_flags = rustc_flags,
            cargo_target_files = profile_files,
            run_from_manifest_dir = True,
            supports_test_execution_caching = True,
        )

    # `//crates/foo` names the member's library or binary when its package has another name.
    alias = member["alias"]
    if alias != None:
        native.alias(name = alias["name"], actual = ":" + alias["actual"], visibility = ["PUBLIC"])
"#;

/// Files of the workspace's directory that no member builds from.
const WORKSPACE_EXCLUDES: &[&str] = &[".git/**", "target/**", "yak-out/**", "YAK"];

fn has_kind(t: &Target, kind: &str) -> bool {
    t.kind.iter().any(|k| k == kind)
}

/// The paths that the Rust files of the member in `dir` include, relative to the workspace's
/// directory and sorted. `includes` holds each include with the path of its file, relative to the
/// workspace's directory.
pub fn resolve_includes(dir: &str, includes: &[(String, IncludedPath)]) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for (file, included) in includes {
        let (base, path) = match included {
            IncludedPath::RelativeToSource(path) => (
                file.rsplit_once('/').map_or("", |(parent, _)| parent),
                path.as_str(),
            ),
            IncludedPath::RelativeToManifestDir(path) => (dir, path.trim_start_matches('/')),
        };
        // A file outside the workspace belongs to no member's package. The scan also finds calls
        // in comments, so such a path fails the crate's compilation instead of the cell, if the
        // crate reads it.
        if let Some(resolved) = workspace_pattern(base, path) {
            paths.insert(resolved);
        }
    }
    paths.into_iter().collect()
}

/// `dir/pattern` with `.` and `..` resolved, relative to the workspace's directory, or `None`
/// when the pattern leaves the workspace.
fn workspace_pattern(dir: &str, pattern: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for component in Path::new(dir).join(pattern).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

/// The rule name of the member's library: the package's name, or `<package>-lib` when a binary of
/// the package has that name.
fn lib_rule(package: &Package) -> String {
    let bin_named_like_package = package
        .targets
        .iter()
        .any(|t| has_kind(t, "bin") && t.name == package.name);
    if bin_named_like_package {
        format!("{}-lib", package.name)
    } else {
        package.name.clone()
    }
}

/// Adds `rule` to the rule names of the member `package`, which must differ.
fn add_rule(
    rules: &mut HashSet<String>,
    package: &Package,
    rule: String,
) -> yak_error::Result<String> {
    if !rules.insert(rule.clone()) {
        return Err(GenerateError::DuplicateTarget(rule, package.name.clone()).into());
    }
    Ok(rule)
}

/// `base/path`, or `path` when `base` is empty.
fn join_dir(base: &str, path: &str) -> String {
    match (base.is_empty(), path.is_empty()) {
        (true, _) => path.to_owned(),
        (false, true) => base.to_owned(),
        (false, false) => format!("{base}/{path}"),
    }
}

fn target_value(
    target: &Target,
    rule: String,
    member_dir: &Path,
    dir: &str,
    extra: Vec<(String, Value)>,
) -> yak_error::Result<Value> {
    let crate_root = relative_to(member_dir, &target.src_path)?;
    let mut fields = vec![
        ("rule".to_owned(), Value::Str(rule)),
        ("crate".to_owned(), Value::str(crate_name(&target.name))),
        (
            "crate_root".to_owned(),
            Value::Str(if dir.is_empty() {
                crate_root
            } else {
                format!("{dir}/{crate_root}")
            }),
        ),
        ("edition".to_owned(), Value::str(&target.edition)),
    ];
    fields.extend(extra);
    Ok(Value::Dict(fields))
}

fn deps_fields(prefix: &str, deps: &Deps, flags: &PlatformFlags) -> Vec<(String, Value)> {
    vec![
        (format!("{prefix}deps"), deps.deps_value()),
        (format!("{prefix}named_deps"), deps.named_deps_value()),
        (format!("{prefix}platform"), deps.platform_value(flags)),
    ]
}

fn manifest_dir(package: &Package) -> &Path {
    Path::new(&package.manifest_path)
        .parent()
        .unwrap_or(Path::new(""))
}

/// The directories of the workspace's members, relative to the workspace's directory.
pub fn member_dirs(metadata: &Metadata) -> yak_error::Result<Vec<String>> {
    let graph = Graph::new(metadata);
    let workspace_root = PathBuf::from(&metadata.workspace_root);
    metadata
        .workspace_members
        .iter()
        .map(|id| {
            let package = graph.package(id)?;
            relative_to(&workspace_root, &manifest_dir(package).to_string_lossy())
        })
        .collect()
}

/// Writes `workspace.bzl` for the members of the workspace. `project_root` is the directory
/// that the workspace's directory is relative to, and `cell` is the name of the cargo cell that
/// holds the third-party packages. `cell_dirs` maps the name of each cell of the project to its
/// directory relative to the project root. `includes` maps the directory of a member, relative to the
/// workspace's directory, to the files of other packages that its Rust files include. The crates
/// compile with the settings of `profile`.
pub fn generate_workspace(
    metadata: &Metadata,
    platforms: &[CargoPlatform],
    project_root: &Path,
    cell: &str,
    cell_dirs: &BTreeMap<String, String>,
    includes: &BTreeMap<String, Vec<IncludedFile>>,
    profile: &Profile,
) -> yak_error::Result<String> {
    let graph = Graph::new(metadata);
    let flags = PlatformFlags::new(platforms);
    let workspace_root = PathBuf::from(&metadata.workspace_root);
    let workspace_dir = relative_to(project_root, &metadata.workspace_root)?;

    let mut members: Vec<(&Package, String)> = Vec::new();
    for id in &metadata.workspace_members {
        let package = graph.package(id)?;
        let dir = relative_to(&workspace_root, &manifest_dir(package).to_string_lossy())?;
        members.push((package, dir));
    }
    members.sort_by(|a, b| a.1.cmp(&b.1));
    let member_packages: HashMap<&str, &Package> =
        members.iter().map(|(p, d)| (d.as_str(), *p)).collect();

    // Each member's targets are in the package of its directory.
    let label = |p: &Package| -> yak_error::Result<String> {
        if graph.is_member(&p.id) {
            let dir = relative_to(&workspace_root, &manifest_dir(p).to_string_lossy())?;
            Ok(format!(
                "//{}:{}",
                join_dir(&workspace_dir, &dir),
                lib_rule(p)
            ))
        } else {
            Ok(format!("{cell}{}", library_label(p)))
        }
    };
    let run_label = |p: &Package| -> yak_error::Result<String> {
        if graph.is_member(&p.id) {
            let dir = relative_to(&workspace_root, &manifest_dir(p).to_string_lossy())?;
            Ok(format!(
                "//{}:{}-build-script-run",
                join_dir(&workspace_dir, &dir),
                p.name
            ))
        } else {
            Ok(format!("{cell}{}", script_run_label(p)))
        }
    };

    // The files that each package that the cell declares, a member or the workspace's root,
    // exports to the members that include them, by package.
    let mut exports: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    let mut values = Vec::new();
    for (package, dir) in &members {
        let node = metadata
            .resolve
            .nodes
            .iter()
            .find(|n| n.id == package.id)
            .ok_or_else(|| GenerateError::UnknownPackage(package.id.clone()))?;
        let member_dir = manifest_dir(package);
        let deps = graph.deps(node, platforms, label, run_label)?;
        let test = deps.normal.merged(&deps.dev);

        if package
            .metadata
            .as_ref()
            .and_then(|m| m.get("yak"))
            .is_some()
        {
            return Err(GenerateError::MetadataMoved(package.name.clone()).into());
        }

        let package_dir = join_dir(&workspace_dir, dir);
        let mut rules: HashSet<String> = HashSet::new();

        // A member builds from the files of its package and the files of other packages that its
        // Rust files include, which their packages export when the cell declares them.
        let srcs_exclude: Vec<String> = if dir.is_empty() {
            WORKSPACE_EXCLUDES.iter().map(|p| (*p).to_owned()).collect()
        } else {
            vec!["target/**".to_owned(), "YAK".to_owned()]
        };
        let mut include = Vec::new();
        for file in includes.get(dir.as_str()).into_iter().flatten() {
            if file.owner == *dir {
                continue;
            }
            let owner_package = join_dir(&workspace_dir, &file.owner);
            let name = match file.path.strip_prefix(&file.owner) {
                Some(rest) if file.owner.is_empty() => rest.to_owned(),
                Some(rest) => rest.trim_start_matches('/').to_owned(),
                None => file.path.clone(),
            };
            include.push((format!("//{owner_package}:{name}"), Value::str(&file.owner)));
            if file.owner.is_empty() || member_packages.contains_key(file.owner.as_str()) {
                exports
                    .entry(owner_package)
                    .or_default()
                    .entry(name)
                    .or_default()
                    .insert(package_dir.clone());
            }
        }

        // Cargo skips the targets whose required features are off, and `cargo test` runs the
        // tests of the library, binaries, and integration tests that do not set `test = false`.
        let features = &node.features;
        let mut test_rules = Vec::new();
        let lib = match lib_target(package) {
            Some(lib) => {
                let unittest = if lib.test {
                    let rule = add_rule(&mut rules, package, format!("{}-unittest", package.name))?;
                    test_rules.push(rule.clone());
                    Value::Str(rule)
                } else {
                    Value::None
                };
                target_value(
                    lib,
                    add_rule(&mut rules, package, lib_rule(package))?,
                    member_dir,
                    dir,
                    vec![
                        ("proc_macro".to_owned(), Value::Bool(lib.is_proc_macro())),
                        ("unittest".to_owned(), unittest),
                    ],
                )?
            }
            None => Value::None,
        };
        let build_script = match package.targets.iter().find(|t| t.is_build_script()) {
            Some(script) => target_value(script, String::new(), member_dir, dir, Vec::new())?,
            None => Value::None,
        };
        let enabled = |kind: &'static str| {
            package
                .targets
                .iter()
                .filter(move |t| has_kind(t, kind) && t.is_enabled(features))
        };
        let mut bin_rules = Vec::new();
        let mut bins = Vec::new();
        for target in enabled("bin") {
            let rule = add_rule(&mut rules, package, target.name.clone())?;
            bin_rules.push(rule.clone());
            let unittest = if !target.test {
                Value::None
            } else if lib_target(package).is_none() && target.name == package.name {
                Value::Str(add_rule(
                    &mut rules,
                    package,
                    format!("{}-unittest", package.name),
                )?)
            } else {
                Value::Str(add_rule(
                    &mut rules,
                    package,
                    format!("{}-{}-unittest", package.name, target.name),
                )?)
            };
            if let Value::Str(rule) = &unittest {
                test_rules.push(rule.clone());
            }
            bins.push(target_value(
                target,
                rule,
                member_dir,
                dir,
                vec![
                    ("name".to_owned(), Value::str(&target.name)),
                    ("unittest".to_owned(), unittest),
                ],
            )?);
        }
        let mut tests = Vec::new();
        for target in enabled("test").filter(|t| t.test) {
            let rule = add_rule(
                &mut rules,
                package,
                format!("{}-{}", package.name, target.name),
            )?;
            test_rules.push(rule.clone());
            tests.push(target_value(target, rule, member_dir, dir, Vec::new())?);
        }
        let mut examples = Vec::new();
        for target in enabled("example") {
            let rule = add_rule(
                &mut rules,
                package,
                format!("{}-example-{}", package.name, target.name),
            )?;
            examples.push(target_value(
                target,
                rule,
                member_dir,
                dir,
                vec![
                    ("name".to_owned(), Value::str(&target.name)),
                    (
                        "crate_types".to_owned(),
                        Value::strs(target.crate_types.iter().cloned()),
                    ),
                ],
            )?);
        }
        let mut rule_names = Vec::new();
        for (key, suffix) in [
            ("build_script_build", "build-script-build"),
            ("build_script_run", "build-script-run"),
        ] {
            rule_names.push((
                key.to_owned(),
                Value::Str(add_rule(
                    &mut rules,
                    package,
                    format!("{}-{suffix}", package.name),
                )?),
            ));
        }

        // `//crates/foo` names `//crates/foo:foo`, which is the library, or else the only binary.
        let dir_name = package_dir.rsplit('/').next().unwrap_or_default();
        let primary = if lib_target(package).is_some() {
            Some(lib_rule(package))
        } else if let [bin] = bin_rules.as_slice() {
            Some(bin.clone())
        } else {
            None
        };
        let alias = match primary {
            Some(actual) if !dir_name.is_empty() && !rules.contains(dir_name) => {
                add_rule(&mut rules, package, dir_name.to_owned())?;
                Value::Dict(vec![
                    ("name".to_owned(), Value::str(dir_name)),
                    ("actual".to_owned(), Value::Str(actual)),
                ])
            }
            _ => Value::None,
        };

        // The build script of a procedural macro gets the settings that its crate compiles with.
        let target = profile.target(&package.name, &package.version, true);
        let host = profile.host(&package.name, &package.version, true);
        let script_env = if lib_target(package).is_some_and(|lib| lib.is_proc_macro()) {
            host.script_env()
        } else {
            target.script_env()
        };
        let profile_value = Value::Dict(vec![
            ("flags".to_owned(), Value::strs(target.rustc_flags())),
            ("host_flags".to_owned(), Value::strs(host.rustc_flags())),
            ("script_env".to_owned(), Value::Dict(script_env)),
        ]);

        let mut env = package_env(package);
        env.push((
            "CARGO_MANIFEST_DIR".to_owned(),
            Value::str(if dir.is_empty() { "." } else { dir.as_str() }),
        ));
        let mut fields = vec![
            ("name".to_owned(), Value::str(&package.name)),
            ("version".to_owned(), Value::str(&package.version)),
            ("dir".to_owned(), Value::str(dir)),
            ("srcs_exclude".to_owned(), Value::strs(srcs_exclude)),
            ("include".to_owned(), Value::Dict(include)),
            (
                "features".to_owned(),
                Value::strs(node.features.iter().cloned()),
            ),
            ("env".to_owned(), Value::Dict(env)),
            ("rules".to_owned(), Value::Dict(rule_names)),
            ("lib".to_owned(), lib),
            ("bins".to_owned(), Value::List(bins)),
            ("tests".to_owned(), Value::List(tests)),
            ("test_rules".to_owned(), Value::strs(test_rules)),
            ("examples".to_owned(), Value::List(examples)),
            ("build_script".to_owned(), build_script),
            ("alias".to_owned(), alias),
            ("profile".to_owned(), profile_value),
        ];
        fields.extend(deps_fields("", &deps.normal, &flags));
        fields.extend(deps_fields("build_", &deps.build, &flags));
        fields.extend(deps_fields("test_", &test, &flags));
        fields.push(("links_deps".to_owned(), deps.links.deps_value()));
        fields.push((
            "links_platform".to_owned(),
            deps.links.platform_deps_value("links_deps"),
        ));
        values.push((package_dir, Value::Dict(fields)));
    }

    let mut out = String::from(
        "# @generated by the cargo cell of yak from `cargo metadata`.\n\n\
         load(\"@prelude//cfg/exec_platform:marker.bzl\", \"get_exec_platform_marker\")\n\
         load(\"@prelude//rust:cargo_buildscript.bzl\", \"buildscript_run\")\n\
         load(\"@prelude//rust:cargo_package.bzl\", \"apply_platform_attrs\", \"cargo\")\n\n\
         _WORKSPACE_DIR = ",
    );
    Value::str(&workspace_dir).render(&mut out);
    out.push_str("\n\n_CELL_DIRS = ");
    Value::Dict(
        cell_dirs
            .iter()
            .map(|(name, dir)| (name.clone(), Value::str(dir)))
            .collect(),
    )
    .render(&mut out);
    out.push_str("\n\n_MEMBERS = ");
    Value::Dict(values).render(&mut out);
    out.push_str("\n\n_EXPORTS = ");
    Value::Dict(
        exports
            .into_iter()
            .map(|(package, files)| {
                (
                    package,
                    Value::Dict(
                        files
                            .into_iter()
                            .map(|(file, users)| (file, Value::strs(users)))
                            .collect(),
                    ),
                )
            })
            .collect(),
    )
    .render(&mut out);
    out.push_str("\n\n_RUSTFLAGS = ");
    Value::strs(flags.all.iter().cloned()).render(&mut out);
    // Cargo compiles a binary and its dependencies with `panic = "abort"` of the profile, and
    // tests with `unwind`, so the binaries transition to a configuration that selects the flag.
    if profile.aborts_on_panic() {
        out.push_str("\n\n_PANIC_FLAGS = ");
        out.push_str(&panic_flags_select());
        out.push_str("\n\n_BINARY_ATTRS = ");
        Value::Dict(vec![(
            "incoming_transition".to_owned(),
            Value::str(PANIC_ABORT_TRANSITION),
        )])
        .render(&mut out);
    } else {
        out.push_str("\n\n_PANIC_FLAGS = []\n\n_BINARY_ATTRS = {}");
    }
    out.push('\n');
    out.push_str(MACRO);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_pattern() {
        assert_eq!(
            workspace_pattern("crates/app", "../../rscript/tsconfig.json").as_deref(),
            Some("rscript/tsconfig.json")
        );
        assert_eq!(
            workspace_pattern("crates/app", "./data/*.txt").as_deref(),
            Some("crates/app/data/*.txt")
        );
        assert_eq!(
            workspace_pattern("", "README.md").as_deref(),
            Some("README.md")
        );
        assert_eq!(workspace_pattern("crates/app", "../../../outside"), None);
        assert_eq!(workspace_pattern("app", "/etc/passwd"), None);
    }
}
