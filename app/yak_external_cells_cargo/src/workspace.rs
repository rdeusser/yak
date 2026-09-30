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

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::graph::CargoPlatform;
use crate::graph::Deps;
use crate::graph::GenerateError;
use crate::graph::Graph;
use crate::graph::crate_name;
use crate::graph::lib_target;
use crate::graph::package_env;
use crate::graph::relative_to;
use crate::metadata::Metadata;
use crate::metadata::Package;
use crate::metadata::Target;
use crate::starlark::Value;
use crate::third_party::library_label;

/// The macro that the workspace's build file calls. `_WORKSPACE_DIR` is the directory of the
/// workspace relative to the project root, and `_MEMBERS` holds the data of each member.
const MACRO: &str = r#"
def cargo_workspace():
    """Declares the targets of the members of the Cargo workspace in this directory."""
    if package_name() != _WORKSPACE_DIR:
        fail("`cargo_workspace()` belongs in the build file of `{}`, the root of the Cargo workspace".format(_WORKSPACE_DIR or "."))
    for member in _MEMBERS:
        _declare_member(member)

def _declare(rule, platform, **kwargs):
    rule(**apply_platform_attrs(platform, kwargs))

def _declare_member(member):
    srcs = glob(member["srcs"], exclude = member["srcs_exclude"])
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
        _declare(
            native.rust_test,
            member["test_platform"],
            name = names["unittest"],
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            srcs = srcs,
            edition = lib["edition"],
            features = member["features"],
            deps = member["test_deps"] + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
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

    for target in member["tests"]:
        _declare(
            native.rust_test,
            member["test_platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            srcs = srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["test_deps"] + own + script_deps,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
        )
"#;

/// Files of the workspace's directory that no member builds from.
const WORKSPACE_EXCLUDES: &[&str] = &[".git/**", "target/**", "yak-out/**", "YAK"];

fn has_kind(t: &Target, kind: &str) -> bool {
    t.kind.iter().any(|k| k == kind)
}

/// The patterns of `[package.metadata.yak] include`, which name files outside the member's
/// directory that the member reads, relative to the member's directory.
fn includes(package: &Package) -> yak_error::Result<Vec<String>> {
    let Some(include) = package
        .metadata
        .as_ref()
        .and_then(|m| m.get("yak"))
        .and_then(|yak| yak.get("include"))
    else {
        return Ok(Vec::new());
    };
    let invalid = || GenerateError::InvalidInclude(package.name.clone());
    include
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|pattern| Ok(pattern.as_str().ok_or_else(invalid)?.to_owned()))
        .collect()
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

/// Writes `workspace.bzl` for the members of the workspace. `project_root` is the directory
/// that the workspace's directory is relative to, and `cell` is the name of the cargo cell that
/// holds the third-party packages.
pub fn generate_workspace(
    metadata: &Metadata,
    platforms: &[CargoPlatform],
    project_root: &Path,
    cell: &str,
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

        // A member builds from the files of its directory, apart from the members inside it,
        // and from the files that `[package.metadata.yak] include` names.
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut srcs: Vec<String> = ["**", "**/.*", "**/.*/**"]
            .iter()
            .map(|p| format!("{prefix}{p}"))
            .collect();
        for pattern in includes(package)? {
            srcs.push(workspace_pattern(dir, &pattern).ok_or_else(|| {
                GenerateError::IncludeOutsideWorkspace(package.name.clone(), pattern.clone())
            })?);
        }
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

        let lib = match lib_target(package) {
            Some(lib) => target_value(
                lib,
                add_rule(names.lib(package))?,
                member_dir,
                dir,
                vec![("proc_macro".to_owned(), Value::Bool(lib.is_proc_macro()))],
            )?,
            None => Value::None,
        };
        let build_script = match package.targets.iter().find(|t| t.is_build_script()) {
            Some(script) => target_value(script, String::new(), member_dir, dir, Vec::new())?,
            None => Value::None,
        };
        let mut bins = Vec::new();
        for target in package.targets.iter().filter(|t| has_kind(t, "bin")) {
            let rule = add_rule(names.bin(package, target))?;
            bins.push(target_value(target, rule, member_dir, dir, Vec::new())?);
        }
        let mut tests = Vec::new();
        for target in package.targets.iter().filter(|t| has_kind(t, "test")) {
            let rule = add_rule(format!("{}-{}", package.name, target.name))?;
            tests.push(target_value(target, rule, member_dir, dir, Vec::new())?);
        }
        let mut rule_names = vec![(
            "unittest".to_owned(),
            Value::Str(add_rule(format!("{}-unittest", package.name))?),
        )];
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
            (
                "features".to_owned(),
                Value::strs(node.features.iter().cloned()),
            ),
            ("env".to_owned(), Value::Dict(env)),
            ("rules".to_owned(), Value::Dict(rule_names)),
            ("lib".to_owned(), lib),
            ("bins".to_owned(), Value::List(bins)),
            ("tests".to_owned(), Value::List(tests)),
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
