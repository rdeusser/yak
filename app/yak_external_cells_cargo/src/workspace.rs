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
use crate::graph::crate_name;
use crate::graph::lib_target;
use crate::graph::package_env;
use crate::graph::relative_to;
use crate::includes::IncludedPath;
use crate::metadata::Metadata;
use crate::metadata::Package;
use crate::metadata::Target;
use crate::third_party::library_label;

/// The macro that the workspace's build file calls. `_WORKSPACE_DIR` is the directory of the
/// workspace relative to the project root, and `_MEMBERS` holds the data of each member.
const MACRO: &str = r#"
def cargo_workspace(include = {}, test_data = {}):
    """Declares the targets of the members of the Cargo workspace in this directory.

    A member builds from the files of its directory and the files that its Rust sources name in
    `include!`, `include_str!`, and `include_bytes!`. `include` maps a member's package name to
    the patterns of other files that its crates read at compile time, and `test_data` to the
    patterns of the files that its tests read at run time, relative to this directory."""
    if package_name() != _WORKSPACE_DIR:
        fail("`cargo_workspace()` belongs in the build file of `{}`, the root of the Cargo workspace".format(_WORKSPACE_DIR or "."))
    names = [member["name"] for member in _MEMBERS]
    for arg, value in [("include", include), ("test_data", test_data)]:
        for name in value:
            if name not in names:
                fail("`{}` names `{}`, which is not a package of the workspace's members".format(arg, name))
    for member in _MEMBERS:
        _declare_member(member, include.get(member["name"], []), test_data.get(member["name"], []))

def _declare(rule, platform, **kwargs):
    rule(**apply_platform_attrs(platform, kwargs))

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
    srcs = glob(member["srcs"], exclude = member["srcs_exclude"])
    own_srcs = {f: None for f in srcs}
    for f in glob(member["include"] + include):
        if f not in own_srcs:
            own_srcs[f] = None
            srcs.append(f)

    # A test reads its member's files and its declared test data from a copy of them, so a read of
    # another file fails.
    test_srcs = srcs + [f for f in glob(test_data) if f not in own_srcs]
    env = dict(member["env"])
    rustc_flags = []
    names = member["rules"]

    # The run target of the build script carries the search paths of the host libraries that the
    # script links to every link of a dependent.
    script_deps = []

    script = member["build_script"]
    if script != None:
        # A build script builds for the execution platform of its run target. The requirement
        # keeps `//...` from building the script and its per-platform aliases for the target
        # platform and for every Cargo platform.
        exec_only = [get_exec_platform_marker()]

        # The build script runs in the member's directory, which holds the member's own files.
        prefix = member["dir"] + "/" if member["dir"] else ""
        native.filegroup(
            name = names["manifest_dir"],
            srcs = {f.removeprefix(prefix): f for f in srcs if f.startswith(prefix)},
        )
        cargo.rust_binary(
            name = names["build_script_build"],
            crate = "build_script_build",
            crate_root = script["crate_root"],
            srcs = srcs,
            edition = script["edition"],
            features = member["features"],
            deps = member["build_deps"],
            named_deps = member["build_named_deps"],
            platform = member["build_platform"],
            env = env,
            target_compatible_with = exec_only,
            visibility = [],
        )
        buildscript_run(
            name = names["build_script_run"],
            buildscript_rule = ":" + names["build_script_build"],
            package_name = member["name"],
            version = member["version"],
            features = member["features"],
            manifest_dir = ":" + names["manifest_dir"],
            buildscript_compatible_with = exec_only,
            env = env,
            rustc_link_lib = True,
            rustc_link_search = True,
        )
        env["OUT_DIR"] = "$(location :{}[out_dir])".format(names["build_script_run"])
        rustc_flags.append("@$(location :{}[rustc_flags])".format(names["build_script_run"]))
        script_deps = [":" + names["build_script_run"]]

    profile_files = select({
        "DEFAULT": _profile_files(member, _LINUX_FILES),
        "prelude//os:macos": _profile_files(member, _MACOS_FILES),
        "prelude//os:windows": _profile_files(member, _WINDOWS_FILES),
    })

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
            srcs = srcs,
            edition = lib["edition"],
            proc_macro = lib["proc_macro"],
            features = member["features"],
            deps = member["deps"] + script_deps,
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            visibility = ["PUBLIC"],
        )
    if lib != None and lib["unittest"] != None:
        _declare(
            native.rust_test,
            member["test_platform"],
            name = lib["unittest"],
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            srcs = test_srcs,
            edition = lib["edition"],
            features = member["features"],
            deps = member["test_deps"] + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            cargo_target_files = profile_files,
            run_from_manifest_dir = True,
        )

    for target in member["bins"]:
        _declare(
            native.rust_binary,
            member["platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            srcs = srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["deps"] + own + script_deps,
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            visibility = ["PUBLIC"],
        )
        if target["unittest"] != None:
            _declare(
                native.rust_test,
                member["test_platform"],
                name = target["unittest"],
                crate = target["crate"],
                crate_root = target["crate_root"],
                srcs = test_srcs,
                edition = target["edition"],
                features = member["features"],
                deps = member["test_deps"] + own + script_deps,
                named_deps = member["test_named_deps"],
                env = env,
                rustc_flags = rustc_flags,
                cargo_target_files = profile_files,
                run_from_manifest_dir = True,
            )

    for example in member["examples"]:
        _declare(
            native.rust_binary if example["crate_types"] == ["bin"] else native.rust_library,
            member["test_platform"],
            name = example["rule"],
            crate = example["crate"],
            crate_root = example["crate_root"],
            srcs = srcs,
            edition = example["edition"],
            features = member["features"],
            deps = member["test_deps"] + own + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
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
            srcs = test_srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["test_deps"] + own + script_deps,
            named_deps = member["test_named_deps"],
            env = test_env,
            rustc_flags = rustc_flags,
            cargo_target_files = profile_files,
            run_from_manifest_dir = True,
        )
"#;

/// Files of the workspace's directory that no member builds from.
const WORKSPACE_EXCLUDES: &[&str] = &[".git/**", "target/**", "yak-out/**", "YAK"];

fn has_kind(t: &Target, kind: &str) -> bool {
    t.kind.iter().any(|k| k == kind)
}

/// The files outside the member's directory `dir` that its Rust files include, relative to the
/// workspace's directory and sorted.
fn member_includes(dir: &str, includes: Option<&Vec<(String, IncludedPath)>>) -> Vec<String> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut paths = BTreeSet::new();
    for (file, included) in includes.into_iter().flatten() {
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
        let Some(resolved) = workspace_pattern(base, path) else {
            continue;
        };
        // A member's own files are its sources already. The root member's sources leave out the
        // directories of the other members.
        if dir.is_empty() || !resolved.starts_with(&prefix) {
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

/// The rule names of the members' targets, which share one package. Each member's library is
/// named after its package, or `<package>-lib` when a binary has that name. A binary keeps its
/// name, so that `yak run //:<binary>` runs it, unless two members have binaries of that name.
struct RuleNames {
    bin_counts: HashMap<String, usize>,
}

impl RuleNames {
    fn new(members: &[&Package]) -> RuleNames {
        let mut bin_counts = HashMap::new();
        for package in members {
            for target in package.targets.iter().filter(|t| has_kind(t, "bin")) {
                *bin_counts.entry(target.name.clone()).or_default() += 1;
            }
        }
        RuleNames { bin_counts }
    }

    fn lib(&self, package: &Package) -> String {
        if self.bin_counts.contains_key(&package.name) {
            format!("{}-lib", package.name)
        } else {
            package.name.clone()
        }
    }

    fn bin(&self, package: &Package, bin: &Target) -> String {
        if self.bin_counts[&bin.name] > 1 {
            format!("{}-{}", package.name, bin.name)
        } else {
            bin.name.clone()
        }
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

fn deps_fields(prefix: &str, deps: &Deps) -> Vec<(String, Value)> {
    vec![
        (format!("{prefix}deps"), deps.deps_value()),
        (format!("{prefix}named_deps"), deps.named_deps_value()),
        (format!("{prefix}platform"), deps.platform_value()),
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
/// holds the third-party packages. `includes` maps the directory of a member, relative to the
/// workspace's directory, to the paths that its Rust files include, each with the file's path
/// relative to the workspace's directory.
pub fn generate_workspace(
    metadata: &Metadata,
    platforms: &[CargoPlatform],
    project_root: &Path,
    cell: &str,
    includes: &BTreeMap<String, Vec<(String, IncludedPath)>>,
) -> yak_error::Result<String> {
    let graph = Graph::new(metadata);
    let workspace_root = PathBuf::from(&metadata.workspace_root);
    let workspace_dir = relative_to(project_root, &metadata.workspace_root)?;

    let mut members: Vec<(&Package, String)> = Vec::new();
    for id in &metadata.workspace_members {
        let package = graph.package(id)?;
        let dir = relative_to(&workspace_root, &manifest_dir(package).to_string_lossy())?;
        members.push((package, dir));
    }
    members.sort_by(|a, b| a.1.cmp(&b.1));
    let names = RuleNames::new(&members.iter().map(|(p, _)| *p).collect::<Vec<_>>());

    let label = |p: &Package| -> yak_error::Result<String> {
        if graph.is_member(&p.id) {
            Ok(format!("//{workspace_dir}:{}", names.lib(p)))
        } else {
            Ok(format!("{cell}{}", library_label(p)))
        }
    };

    let mut rules: HashSet<String> = HashSet::new();
    let mut add_rule = |rule: String| -> yak_error::Result<String> {
        if !rules.insert(rule.clone()) {
            return Err(GenerateError::DuplicateTarget(rule).into());
        }
        Ok(rule)
    };

    let mut values = Vec::new();
    for (package, dir) in &members {
        let node = metadata
            .resolve
            .nodes
            .iter()
            .find(|n| n.id == package.id)
            .ok_or_else(|| GenerateError::UnknownPackage(package.id.clone()))?;
        let member_dir = manifest_dir(package);
        let deps = graph.deps(node, platforms, label)?;
        let test = deps.normal.merged(&deps.dev);

        if package
            .metadata
            .as_ref()
            .and_then(|m| m.get("yak"))
            .is_some()
        {
            return Err(GenerateError::MetadataMoved(package.name.clone()).into());
        }

        // A member builds from the files of its directory, apart from the members inside it,
        // and from the files outside it that its Rust files include.
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let srcs: Vec<String> = ["**", "**/.*", "**/.*/**"]
            .iter()
            .map(|p| format!("{prefix}{p}"))
            .collect();
        let include = member_includes(dir, includes.get(dir.as_str()));
        let mut srcs_exclude: Vec<String> = if dir.is_empty() {
            WORKSPACE_EXCLUDES.iter().map(|p| (*p).to_owned()).collect()
        } else {
            vec![format!("{prefix}target/**")]
        };
        srcs_exclude.extend(
            members
                .iter()
                .filter(|(_, other)| other != dir && other.starts_with(&prefix))
                .map(|(_, other)| format!("{other}/**")),
        );

        // Cargo skips the targets whose required features are off, and `cargo test` runs the
        // tests of the library, binaries, and integration tests that do not set `test = false`.
        let features = &node.features;
        let lib = match lib_target(package) {
            Some(lib) => {
                let unittest = if lib.test {
                    Value::Str(add_rule(format!("{}-unittest", package.name))?)
                } else {
                    Value::None
                };
                target_value(
                    lib,
                    add_rule(names.lib(package))?,
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
        let mut bins = Vec::new();
        for target in enabled("bin") {
            let rule = add_rule(names.bin(package, target))?;
            let unittest = if !target.test {
                Value::None
            } else if lib_target(package).is_none() && target.name == package.name {
                Value::Str(add_rule(format!("{}-unittest", package.name))?)
            } else {
                Value::Str(add_rule(format!(
                    "{}-{}-unittest",
                    package.name, target.name
                ))?)
            };
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
            let rule = add_rule(format!("{}-{}", package.name, target.name))?;
            tests.push(target_value(target, rule, member_dir, dir, Vec::new())?);
        }
        let mut examples = Vec::new();
        for target in enabled("example") {
            let rule = add_rule(format!("{}-example-{}", package.name, target.name))?;
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
            ("manifest_dir", "manifest-dir"),
            ("build_script_build", "build-script-build"),
            ("build_script_run", "build-script-run"),
        ] {
            rule_names.push((
                key.to_owned(),
                Value::Str(add_rule(format!("{}-{suffix}", package.name))?),
            ));
        }

        let mut env = package_env(package);
        env.push((
            "CARGO_MANIFEST_DIR".to_owned(),
            Value::str(if dir.is_empty() { "." } else { dir.as_str() }),
        ));
        let mut fields = vec![
            ("name".to_owned(), Value::str(&package.name)),
            ("version".to_owned(), Value::str(&package.version)),
            ("dir".to_owned(), Value::str(dir)),
            ("srcs".to_owned(), Value::strs(srcs)),
            ("srcs_exclude".to_owned(), Value::strs(srcs_exclude)),
            ("include".to_owned(), Value::strs(include)),
            (
                "features".to_owned(),
                Value::strs(node.features.iter().cloned()),
            ),
            ("env".to_owned(), Value::Dict(env)),
            ("rules".to_owned(), Value::Dict(rule_names)),
            ("lib".to_owned(), lib),
            ("bins".to_owned(), Value::List(bins)),
            ("tests".to_owned(), Value::List(tests)),
            ("examples".to_owned(), Value::List(examples)),
            ("build_script".to_owned(), build_script),
        ];
        fields.extend(deps_fields("", &deps.normal));
        fields.extend(deps_fields("build_", &deps.build));
        fields.extend(deps_fields("test_", &test));
        values.push(Value::Dict(fields));
    }

    let mut out = String::from(
        "# @generated by the cargo cell of yak from `cargo metadata`.\n\n\
         load(\"@prelude//cfg/exec_platform:marker.bzl\", \"get_exec_platform_marker\")\n\
         load(\"@prelude//rust:cargo_buildscript.bzl\", \"buildscript_run\")\n\
         load(\"@prelude//rust:cargo_package.bzl\", \"apply_platform_attrs\", \"cargo\")\n\n\
         _WORKSPACE_DIR = ",
    );
    Value::str(&workspace_dir).render(&mut out);
    out.push_str("\n\n_MEMBERS = ");
    Value::List(values).render(&mut out);
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
