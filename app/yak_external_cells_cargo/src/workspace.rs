/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Writes `workspace.bzl`, which holds the data of each workspace member and the macro that
//! declares a member's targets from it. A member's build file only calls the macro.

use std::path::Path;

use crate::graph::CargoPlatform;
use crate::graph::Deps;
use crate::graph::Graph;
use crate::graph::crate_name;
use crate::graph::lib_target;
use crate::graph::package_env;
use crate::graph::relative_to;
use crate::graph::target_id;
use crate::metadata::Metadata;
use crate::metadata::Package;
use crate::metadata::Target;
use crate::starlark::Value;

/// The macro that a member's build file calls. `_MEMBERS` maps the directory of each member,
/// relative to the project root, to its data.
const MACRO: &str = r#"
def cargo_workspace_member():
    """Declares the targets of the Cargo workspace member in this directory."""
    member = _MEMBERS.get(package_name())
    if member == None:
        fail("`{}` is not the directory of a member of the Cargo workspace".format(package_name()))

    srcs = glob(["**"], exclude = ["target/**", "YAK"])
    env = dict(member["env"])
    rustc_flags = []

    script = member["build_script"]
    if script != None:
        native.filegroup(name = member["name"] + "-manifest-dir", srcs = srcs)
        cargo.rust_binary(
            name = member["name"] + "-build-script-build",
            crate = "build_script_build",
            crate_root = script["crate_root"],
            srcs = srcs,
            edition = script["edition"],
            features = member["features"],
            deps = member["build_deps"],
            named_deps = member["build_named_deps"],
            platform = member["build_platform"],
            env = env,
            visibility = [],
        )
        buildscript_run(
            name = member["name"] + "-build-script-run",
            buildscript_rule = ":" + member["name"] + "-build-script-build",
            package_name = member["name"],
            version = member["version"],
            features = member["features"],
            manifest_dir = ":" + member["name"] + "-manifest-dir",
            env = env,
        )
        env["OUT_DIR"] = "$(location :{}-build-script-run[out_dir])".format(member["name"])
        rustc_flags.append("@$(location :{}-build-script-run[rustc_flags])".format(member["name"]))

    def declare(rule, platform, **kwargs):
        rule(**apply_platform_attrs(platform, kwargs))

    lib = member["lib"]
    own = []
    if lib != None:
        own = [":" + lib["rule"]]
        declare(
            native.rust_library,
            member["platform"],
            name = lib["rule"],
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            srcs = srcs,
            edition = lib["edition"],
            proc_macro = lib["proc_macro"],
            features = member["features"],
            deps = member["deps"],
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            visibility = ["PUBLIC"],
        )
        declare(
            native.rust_test,
            member["test_platform"],
            name = lib["rule"] + "-unittest",
            crate = lib["crate"],
            crate_root = lib["crate_root"],
            srcs = srcs,
            edition = lib["edition"],
            features = member["features"],
            deps = member["test_deps"],
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
        )

    for target in member["bins"]:
        declare(
            native.rust_binary,
            member["platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            srcs = srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["deps"] + own,
            named_deps = member["named_deps"],
            env = env,
            rustc_flags = rustc_flags,
            visibility = ["PUBLIC"],
        )

    for target in member["tests"]:
        declare(
            native.rust_test,
            member["test_platform"],
            name = target["rule"],
            crate = target["crate"],
            crate_root = target["crate_root"],
            srcs = srcs,
            edition = target["edition"],
            features = member["features"],
            deps = member["test_deps"] + own,
            named_deps = member["test_named_deps"],
            env = env,
            rustc_flags = rustc_flags,
        )
"#;

fn has_kind(t: &Target, kind: &str) -> bool {
    t.kind.iter().any(|k| k == kind)
}

/// The rule name of a member's library. It is the package name, unless a binary of the package
/// has that name, because binaries keep their names so that `yak run` finds them.
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

fn target_value(
    target: &Target,
    rule: String,
    dir: &Path,
    extra: Vec<(String, Value)>,
) -> yak_error::Result<Value> {
    let mut fields = vec![
        ("rule".to_owned(), Value::Str(rule)),
        ("crate".to_owned(), Value::str(crate_name(&target.name))),
        (
            "crate_root".to_owned(),
            Value::Str(relative_to(dir, &target.src_path)?),
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

/// Writes `workspace.bzl` for the members of the workspace. `project_root` is the directory
/// that member directories are relative to, and `cell` is the name of the cargo cell that holds
/// the third-party packages.
pub fn generate_workspace(
    metadata: &Metadata,
    platforms: &[CargoPlatform],
    project_root: &Path,
    cell: &str,
) -> yak_error::Result<String> {
    let graph = Graph::new(metadata);
    let member_dir = |p: &Package| -> yak_error::Result<String> {
        let manifest_dir = Path::new(&p.manifest_path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .into_owned();
        relative_to(project_root, &manifest_dir)
    };
    let label = |p: &Package| -> yak_error::Result<String> {
        if graph.is_member(&p.id) {
            Ok(format!("//{}:{}", member_dir(p)?, lib_rule(p)))
        } else {
            Ok(format!("{cell}//:{}", target_id(p)))
        }
    };

    let mut members = Vec::new();
    for node in &metadata.resolve.nodes {
        if !graph.is_member(&node.id) {
            continue;
        }
        let package = graph.package(&node.id)?;
        let dir_rel = member_dir(package)?;
        let dir = Path::new(&package.manifest_path)
            .parent()
            .unwrap_or(Path::new(""));
        let deps = graph.deps(node, platforms, label)?;
        let test = deps.normal.merged(&deps.dev);

        let lib = match lib_target(package) {
            Some(lib) => target_value(
                lib,
                lib_rule(package),
                dir,
                vec![("proc_macro".to_owned(), Value::Bool(lib.is_proc_macro()))],
            )?,
            None => Value::None,
        };
        let build_script = match package.targets.iter().find(|t| t.is_build_script()) {
            Some(script) => target_value(script, String::new(), dir, Vec::new())?,
            None => Value::None,
        };
        let targets_of = |kind: &str| -> yak_error::Result<Value> {
            Ok(Value::List(
                package
                    .targets
                    .iter()
                    .filter(|t| has_kind(t, kind))
                    .map(|t| target_value(t, t.name.clone(), dir, Vec::new()))
                    .collect::<yak_error::Result<_>>()?,
            ))
        };

        let mut env = package_env(package);
        env.push(("CARGO_MANIFEST_DIR".to_owned(), Value::str(".")));
        let mut fields = vec![
            ("name".to_owned(), Value::str(&package.name)),
            ("version".to_owned(), Value::str(&package.version)),
            (
                "features".to_owned(),
                Value::strs(node.features.iter().cloned()),
            ),
            ("env".to_owned(), Value::Dict(env)),
            ("lib".to_owned(), lib),
            ("bins".to_owned(), targets_of("bin")?),
            ("tests".to_owned(), targets_of("test")?),
            ("build_script".to_owned(), build_script),
        ];
        fields.extend(deps_fields("", &deps.normal));
        fields.extend(deps_fields("build_", &deps.build));
        fields.extend(deps_fields("test_", &test));
        members.push((dir_rel, Value::Dict(fields)));
    }
    members.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from(
        "# @generated by the cargo cell of yak from `cargo metadata`.\n\n\
         load(\"@prelude//rust:cargo_buildscript.bzl\", \"buildscript_run\")\n\
         load(\"@prelude//rust:cargo_package.bzl\", \"apply_platform_attrs\", \"cargo\")\n\n\
         _MEMBERS = ",
    );
    Value::Dict(members).render(&mut out);
    out.push('\n');
    out.push_str(MACRO);
    Ok(out)
}
