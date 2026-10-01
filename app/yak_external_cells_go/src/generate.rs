/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Writes the files of a go cell from the `go list` output of a Go module.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use yak_external_cells_starlark::Value;
use yak_external_cells_starlark::call;

use crate::list::Package;
use crate::platform::PLATFORMS;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum GenerateError {
    #[error("`go list` reported no package of the main module")]
    NoMainModule,
    #[error(
        "`{0}` is replaced by a local directory, which a go cell does not support. Its files would not be tracked."
    )]
    LocalReplacement(String),
    #[error(
        "The third-party package `{0}` imports `{1}` of the main module, which a go cell does not support"
    )]
    ImportsMainModule(String, String),
    #[error("The Go packages `{0}` and `{1}` of the module would both declare the target `{2}`")]
    DuplicateTarget(String, String, String),
}

/// ModuleInputs is what a go cell knows about its Go module.
pub struct ModuleInputs<'a> {
    /// The module's directory relative to the root of its cell, empty for the cell's root.
    pub module_dir: &'a str,
    /// The name of the cell, which the module's build files use in labels.
    pub cell: &'a str,
    /// The packages that `go list` reported for each of `PLATFORMS`, in its order.
    pub listings: &'a [Vec<Package>],
    /// The directories below the module's root, relative to it, that have a build file, each
    /// with whether that file calls `go_package()`.
    pub build_files: &'a BTreeMap<String, bool>,
}

/// GoCell is the generated contents of a go cell.
pub struct GoCell {
    /// The cell's root build file, which holds an alias for each third-party package.
    pub root_build_file: String,
    /// `module.bzl`, which holds the first-party packages and the macros that declare them.
    pub module_file: String,
    /// Sorted by directory.
    pub modules: Vec<ModuleVersion>,
}

/// ModuleVersion is a third-party module version, a package of the cell.
pub struct ModuleVersion {
    /// `<module path>@<version>`.
    pub dir: String,
    /// The absolute path of the directory that Go put the module's files in.
    pub source_dir: String,
    pub build_file: String,
}

/// The file patterns of a Go package's sources, relative to its directory. The prelude's Go
/// rules pick the files that build for the target platform.
const SRC_PATTERNS: &[&str] = &[
    "*.go", "*.s", "*.S", "*.c", "*.h", "*.cc", "*.cpp", "*.cxx", "*.hh", "*.hpp", "*.m", "*.syso",
];

/// A package as `go list` reported it on each platform.
struct Merged<'a> {
    /// The package on the first platform that has it, for the facts that do not depend on the
    /// platform.
    package: &'a Package,
    platforms: Vec<Option<&'a Package>>,
}

impl<'a> Merged<'a> {
    fn present(&self) -> impl Iterator<Item = &'a Package> + '_ {
        self.platforms.iter().flatten().copied()
    }

    /// The files that any platform embeds, from `files`.
    fn embeds(&self, files: impl Fn(&Package) -> Vec<&String>) -> BTreeSet<String> {
        self.present()
            .flat_map(|p| files(p).into_iter().cloned())
            .collect()
    }

    fn has_tests(&self) -> bool {
        self.present()
            .any(|p| !p.test_go_files.is_empty() || !p.x_test_go_files.is_empty())
    }

    fn has_sources(&self) -> bool {
        self.present()
            .any(|p| !p.go_files.is_empty() || !p.cgo_files.is_empty())
    }
}

fn merge(listings: &[Vec<Package>]) -> BTreeMap<&str, Merged<'_>> {
    let mut merged: BTreeMap<&str, Merged<'_>> = BTreeMap::new();
    for (i, listing) in listings.iter().enumerate() {
        for package in listing {
            merged
                .entry(&package.import_path)
                .or_insert_with(|| Merged {
                    package,
                    platforms: vec![None; listings.len()],
                })
                .platforms[i] = Some(package);
        }
    }
    merged
}

/// The import path of `import_path` relative to the module path `module`, empty for the
/// module's root package.
fn relative(import_path: &str, module: &str) -> String {
    if import_path == module {
        String::new()
    } else {
        import_path
            .strip_prefix(module)
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(import_path)
            .to_owned()
    }
}

fn join(dir: &str, rest: &str) -> String {
    match (dir.is_empty(), rest.is_empty()) {
        (true, _) => rest.to_owned(),
        (_, true) => dir.to_owned(),
        _ => format!("{dir}/{rest}"),
    }
}

/// The last element of a module path that is not a major version suffix, which `go build`
/// names a binary after.
fn base_name(path: &str) -> &str {
    let mut elements = path.rsplit('/');
    let last = elements.next().unwrap_or(path);
    let is_major =
        last.len() > 1 && last.starts_with('v') && last[1..].bytes().all(|b| b.is_ascii_digit());
    match elements.next() {
        Some(previous) if is_major => previous,
        _ => last,
    }
}

fn last_element(dir: &str) -> &str {
    dir.rsplit('/').next().unwrap_or(dir)
}

fn render(value: &Value) -> String {
    let mut out = String::new();
    value.render(&mut out);
    out
}

/// The labels that a package depends on: the ones every platform that has the package needs,
/// and a `select` over the prelude's OS and CPU constraints for the others.
fn deps_value(per_platform: &[Option<BTreeSet<String>>]) -> Value {
    let present: Vec<&BTreeSet<String>> = per_platform.iter().flatten().collect();
    let common: BTreeSet<String> = match present.split_first() {
        Some((first, rest)) => first
            .iter()
            .filter(|label| rest.iter().all(|set| set.contains(*label)))
            .cloned()
            .collect(),
        None => BTreeSet::new(),
    };
    let mut by_os: BTreeMap<&str, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    for (platform, set) in PLATFORMS.iter().zip(per_platform) {
        let extra: Vec<String> = set
            .iter()
            .flatten()
            .filter(|label| !common.contains(*label))
            .cloned()
            .collect();
        if !extra.is_empty() {
            by_os
                .entry(platform.os)
                .or_default()
                .insert(platform.cpu, extra);
        }
    }
    let common = Value::strs(common);
    if by_os.is_empty() {
        return common;
    }
    let os_branches = by_os
        .into_iter()
        .map(|(os, by_cpu)| {
            let cpus_of_os = PLATFORMS.iter().filter(|p| p.os == os).count();
            let first = by_cpu.values().next().cloned().unwrap_or_default();
            let branch = if by_cpu.len() == cpus_of_os && by_cpu.values().all(|v| *v == first) {
                Value::strs(first)
            } else {
                let mut entries: Vec<(String, Value)> = by_cpu
                    .into_iter()
                    .map(|(cpu, labels)| (cpu.to_owned(), Value::strs(labels)))
                    .collect();
                entries.push(("DEFAULT".to_owned(), Value::List(Vec::new())));
                Value::Expr(format!("select({})", render(&Value::Dict(entries))))
            };
            (os.to_owned(), branch)
        })
        .chain([("DEFAULT".to_owned(), Value::List(Vec::new()))])
        .collect();
    let select = format!("select({})", render(&Value::Dict(os_branches)));
    match &common {
        Value::List(items) if items.is_empty() => Value::Expr(select),
        _ => Value::Expr(format!("{} + {select}", render(&common))),
    }
}

/// A target that `go_module()` or `go_package()` declares for a first-party package.
struct FirstParty<'a> {
    merged: &'a Merged<'a>,
    /// The directory of the build file that declares the package's targets, the nearest one at
    /// or above the package's directory, relative to the module's root.
    owner: String,
    /// The package's directory relative to `owner`.
    dir: String,
    name: String,
}

impl FirstParty<'_> {
    fn is_main(&self) -> bool {
        self.merged.package.name == "main"
    }
}

/// The directory of the nearest build file at or above `dir`, the module's root if no other.
fn owner<'b>(dir: &str, build_files: &'b BTreeMap<String, bool>) -> &'b str {
    build_files
        .keys()
        .filter(|b| dir == b.as_str() || dir.starts_with(&format!("{b}/")))
        .max_by_key(|b| b.len())
        .map_or("", |b| b.as_str())
}

/// Names the targets of the first-party packages. The package in the directory of its build
/// file is named by the last element of that directory, or of the module path at the module's
/// root. Any other library is named by its directory relative to the build file, and a binary
/// by the last element of its directory, as `go build` names it, unless another target of the
/// build file has that name.
fn first_party<'a>(
    merged: &'a BTreeMap<&str, Merged<'a>>,
    module_path: &str,
    build_files: &BTreeMap<String, bool>,
) -> yak_error::Result<Vec<FirstParty<'a>>> {
    let mut packages: Vec<FirstParty<'a>> = merged
        .values()
        .filter(|m| m.package.module.as_ref().is_some_and(|m| m.main))
        .map(|m| {
            let module_dir = relative(&m.package.import_path, module_path);
            let owner = owner(&module_dir, build_files).to_owned();
            let dir = relative(&module_dir, &owner);
            let name = if dir.is_empty() {
                if owner.is_empty() {
                    base_name(module_path).to_owned()
                } else {
                    last_element(&owner).to_owned()
                }
            } else if m.package.name == "main" {
                last_element(&dir).to_owned()
            } else {
                dir.clone()
            };
            FirstParty {
                merged: m,
                owner,
                dir,
                name,
            }
        })
        .collect();
    if packages.is_empty() {
        return Err(GenerateError::NoMainModule.into());
    }

    // Binaries whose name another target of the build file has take their directory.
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for p in &packages {
        *counts.entry((p.owner.clone(), p.name.clone())).or_default() += 1;
    }
    for p in &mut packages {
        if p.is_main() && !p.dir.is_empty() && counts[&(p.owner.clone(), p.name.clone())] > 1 {
            p.name = p.dir.clone();
        }
    }

    let mut seen: BTreeMap<(String, String), &str> = BTreeMap::new();
    for p in &packages {
        for name in [p.name.clone(), format!("{}-test", p.name)] {
            if let Some(other) = seen.insert(
                (p.owner.clone(), name.clone()),
                &p.merged.package.import_path,
            ) {
                return Err(GenerateError::DuplicateTarget(
                    other.to_owned(),
                    p.merged.package.import_path.clone(),
                    name,
                )
                .into());
            }
        }
    }
    Ok(packages)
}

/// A `glob` of the source files of the Go package in `dir`, apart from its tests.
fn srcs_glob(dir: &str) -> String {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let test_files = Value::strs([format!("{prefix}*_test.go")]);
    let patterns = Value::strs(SRC_PATTERNS.iter().map(|p| format!("{prefix}{p}")));
    format!(
        "glob({}, exclude = {})",
        render(&patterns),
        render(&test_files)
    )
}

/// The macros of `module.bzl`. `_MODULE_DIR` is the module's directory relative to the root of
/// its cell, `_SRC_PATTERNS` holds `SRC_PATTERNS`, `_PACKAGES` maps the directory of each first-party package, relative to the
/// module's root, to its data, and `_MISSING_GO_PACKAGE` lists the directories whose build file
/// owns a Go package but does not call `go_package()`.
const MACROS: &str = r#"
def go_module(test_data = {}):
    """Declares the targets of the Go packages of the module in this directory. The build file
    of a directory below declares the packages in that directory and the directories below it
    with `go_package()`.

    A test reads the files of its package's directory, apart from the directories of other
    packages, and runs in that directory, as with `go test`. `test_data` maps the directory of a
    package, relative to this one, to the patterns of the other files its tests read, such as
    `{"pkg/verify": ["pkg/testdata/**"]}`."""
    if package_name() != _MODULE_DIR:
        fail("`go_module()` belongs in the build file of `{}`, the directory of the Go module's `go.mod`".format(_MODULE_DIR or "."))
    if _MISSING_GO_PACKAGE:
        fail("The build file of `{}` does not call `go_package()`, which declares the targets of the Go packages in its directory and the directories below it".format(_MISSING_GO_PACKAGE[0]))
    _declare_owned("", test_data)

def go_package(test_data = {}):
    """Declares the targets of the Go packages in this directory and the directories below it
    that have no build file of their own. `test_data` is as for `go_module()`."""
    owner = package_name()
    if _MODULE_DIR:
        owner = owner.removeprefix(_MODULE_DIR + "/")
    if not _declare_owned(owner, test_data):
        fail("`{}` and the directories below it hold no Go package of the module in `{}`".format(package_name(), _MODULE_DIR or "."))

def _declare_owned(owner, test_data):
    owned = [p for p in _PACKAGES.values() if p["owner"] == owner]
    dirs = [p["dir"] for p in owned]
    for dir in test_data:
        if dir not in dirs:
            fail("`test_data` names `{}`, which holds no Go package of this build file".format(dir))
    for package in owned:
        dir = package["dir"]
        prefix = dir + "/" if dir else ""

        # The directories of the build file's other packages below this one.
        nested = [d + "/**" for d in dirs if d != dir and d.startswith(prefix)]
        resources = glob([prefix + "**"], exclude = nested) + glob(test_data.get(dir, []))
        _declare(package, dir, resources)
    return len(owned) > 0

def _declare(package, dir, resources):
    prefix = dir + "/" if dir else ""
    srcs = glob([prefix + p for p in _SRC_PATTERNS], exclude = [prefix + "*_test.go"])
    embeds = {prefix + f: prefix + f for f in package["embeds"]}
    name = package["name"]
    if package["kind"] == "binary":
        native.go_binary(
            name = name,
            package_root = dir,
            srcs = srcs,
            embed_srcs = embeds,
            deps = package["deps"],
        )
    elif package["kind"] == "library":
        native.go_library(
            name = name,
            package_name = package["import_path"],
            package_root = dir,
            srcs = srcs,
            embed_srcs = embeds,
            deps = package["deps"],
            visibility = ["PUBLIC"],
        )
    test = package["test"]
    if test == None:
        return
    test_embeds = {prefix + f: prefix + f for f in test["embeds"]}
    if package["kind"] == "library":
        native.go_test(
            name = name + "-test",
            package_root = dir,
            srcs = glob([prefix + "*_test.go"]),
            embed_srcs = test_embeds,
            target_under_test = ":" + name,
            external_tests_only = test["external_only"],
            deps = test["deps"],
            resources = resources,
            working_directory = dir,
        )
    else:
        # A test of a `main` package, or of a package with only test files, compiles the
        # package's files with its tests.
        native.go_test(
            name = name + "-test",
            package_name = package["import_path"],
            package_root = dir,
            srcs = glob([prefix + p for p in _SRC_PATTERNS]),
            embed_srcs = test_embeds,
            deps = package["deps"] + test["deps"],
            resources = resources,
            working_directory = dir,
        )
"#;

/// Writes the files of the go cell of the module whose `go list` output `inputs` holds.
pub fn generate(inputs: &ModuleInputs<'_>) -> yak_error::Result<GoCell> {
    let merged = merge(inputs.listings);
    let module_path = merged
        .values()
        .find_map(|m| m.package.module.as_ref().filter(|m| m.main))
        .map(|m| m.path.clone())
        .ok_or(GenerateError::NoMainModule)?;
    let first_party = first_party(&merged, &module_path, inputs.build_files)?;
    let first_party_labels: BTreeMap<&str, String> = first_party
        .iter()
        .map(|p| {
            let package_dir = join(inputs.module_dir, &p.owner);
            (
                p.merged.package.import_path.as_str(),
                format!("//{package_dir}:{}", p.name),
            )
        })
        .collect();

    // The directory in the cell of each third-party module version.
    let mut module_dirs: BTreeMap<&str, String> = BTreeMap::new();
    for m in merged.values() {
        let Some(module) = m.package.module.as_ref().filter(|m| !m.main) else {
            continue;
        };
        let source = m.package.source_module().unwrap_or(module);
        if source.version.is_empty() {
            return Err(GenerateError::LocalReplacement(module.path.clone()).into());
        }
        module_dirs.insert(
            &m.package.import_path,
            format!("{}@{}", source.path, source.version),
        );
    }

    // The labels that a package's imports on each platform need, with `label` naming each
    // imported package. Imports of the standard library and of `C` have no label.
    let deps = |m: &Merged<'_>,
                imports: &dyn Fn(&Package) -> Vec<&String>,
                label: &dyn Fn(&str) -> yak_error::Result<Option<String>>|
     -> yak_error::Result<Value> {
        let per_platform = m
            .platforms
            .iter()
            .map(|p| {
                p.map(|p| {
                    imports(p)
                        .into_iter()
                        .filter(|i| **i != m.package.import_path && merged.contains_key(i.as_str()))
                        .filter_map(|i| label(i).transpose())
                        .collect::<yak_error::Result<BTreeSet<String>>>()
                })
                .transpose()
            })
            .collect::<yak_error::Result<Vec<_>>>()?;
        Ok(deps_value(&per_platform))
    };

    let cell = inputs.cell;
    let module_label = |import: &str| -> yak_error::Result<Option<String>> {
        Ok(Some(match module_dirs.get(import) {
            Some(dir) => format!("{cell}//{dir}:{import}"),
            None => first_party_labels[import].clone(),
        }))
    };

    let mut packages = Vec::new();
    for p in &first_party {
        let m = p.merged;
        let kind = if p.is_main() {
            "binary"
        } else if m.has_sources() {
            "library"
        } else {
            "tests"
        };
        let embeds = m.embeds(|p| p.embed_files.iter().collect());
        let lib_deps = deps(m, &|p| p.imports.iter().collect(), &module_label)?;
        let test = if m.has_tests() {
            // A library's test takes the library's dependencies through `target_under_test`,
            // and the test of any other package lists them with its own.
            let test_deps = if kind == "library" {
                deps(
                    m,
                    &|p| p.test_imports.iter().chain(&p.x_test_imports).collect(),
                    &module_label,
                )?
            } else {
                deps(
                    m,
                    &|p| {
                        p.test_imports
                            .iter()
                            .chain(&p.x_test_imports)
                            .filter(|i| !p.imports.contains(i))
                            .collect()
                    },
                    &module_label,
                )?
            };
            let test_embeds = m.embeds(|p| {
                p.embed_files
                    .iter()
                    .chain(&p.test_embed_files)
                    .chain(&p.x_test_embed_files)
                    .collect()
            });
            // Without test files of its own, the package links as its library target builds it.
            let external_only = m
                .present()
                .all(|p| p.test_go_files.is_empty() && !p.x_test_go_files.is_empty());
            Value::Dict(vec![
                ("embeds".to_owned(), Value::strs(test_embeds)),
                ("deps".to_owned(), test_deps),
                ("external_only".to_owned(), Value::Bool(external_only)),
            ])
        } else {
            Value::None
        };
        packages.push((
            join(&p.owner, &p.dir),
            Value::Dict(vec![
                ("import_path".to_owned(), Value::str(&m.package.import_path)),
                ("kind".to_owned(), Value::str(kind)),
                ("name".to_owned(), Value::str(&p.name)),
                ("owner".to_owned(), Value::str(&p.owner)),
                ("dir".to_owned(), Value::str(&p.dir)),
                ("embeds".to_owned(), Value::strs(embeds)),
                ("deps".to_owned(), lib_deps),
                ("test".to_owned(), test),
            ]),
        ));
    }
    let missing: Vec<String> = inputs
        .build_files
        .iter()
        .filter(|(dir, calls)| !**calls && first_party.iter().any(|p| &&p.owner == dir))
        .map(|(dir, _)| join(inputs.module_dir, dir))
        .collect();

    let mut module_file = String::from("# @generated by the go cell from `go list`.\n\n");
    module_file.push_str(&format!(
        "_MODULE_DIR = {}\n\n_SRC_PATTERNS = {}\n\n_MISSING_GO_PACKAGE = {}\n\n_PACKAGES = ",
        render(&Value::str(inputs.module_dir)),
        render(&Value::strs(SRC_PATTERNS.iter().copied())),
        render(&Value::strs(missing)),
    ));
    let mut packages_value = String::from("{\n");
    for (dir, value) in &packages {
        packages_value.push_str(&format!(
            "    {}: {},\n",
            render(&Value::str(dir)),
            render(value)
        ));
    }
    packages_value.push('}');
    module_file.push_str(&packages_value);
    module_file.push('\n');
    module_file.push_str(MACROS);

    // The third-party packages, by module version.
    let mut versions: BTreeMap<&str, (String, String)> = BTreeMap::new();
    let mut root_build_file = String::from("# @generated by the go cell from `go list`.\n\n");
    for m in merged.values() {
        let Some(dir) = module_dirs.get(m.package.import_path.as_str()) else {
            continue;
        };
        let module = m
            .package
            .module
            .as_ref()
            .expect("a third-party package has a module");
        let source = m.package.source_module().unwrap_or(module);
        let (_, build_file) = versions.entry(dir).or_insert_with(|| {
            (
                source.dir.clone(),
                String::from("# @generated by the go cell from `go list`.\n\n"),
            )
        });
        let package_dir = relative(&m.package.import_path, &module.path);
        let lib_deps = deps(
            m,
            &|p| p.imports.iter().collect(),
            &|i| match module_dirs.get(i) {
                Some(dir) => Ok(Some(format!("//{dir}:{i}"))),
                None => Err(GenerateError::ImportsMainModule(
                    m.package.import_path.clone(),
                    i.to_owned(),
                )
                .into()),
            },
        )?;
        let embeds = m.embeds(|p| p.embed_files.iter().collect());
        let mut kwargs = vec![
            ("name", Value::str(&m.package.import_path)),
            ("package_name", Value::str(&m.package.import_path)),
            ("package_root", Value::str(&package_dir)),
            ("srcs", Value::Expr(srcs_glob(&package_dir))),
        ];
        if !embeds.is_empty() {
            kwargs.push((
                "embed_srcs",
                Value::Dict(
                    embeds
                        .iter()
                        .map(|e| {
                            let path = join(&package_dir, e);
                            (path.clone(), Value::str(path))
                        })
                        .collect(),
                ),
            ));
        }
        kwargs.push(("deps", lib_deps));
        kwargs.push(("visibility", Value::strs(["PUBLIC"])));
        call(build_file, "go_library", &kwargs);
        call(
            &mut root_build_file,
            "alias",
            &[
                ("name", Value::str(&m.package.import_path)),
                (
                    "actual",
                    Value::str(format!("//{dir}:{}", m.package.import_path)),
                ),
                ("visibility", Value::strs(["PUBLIC"])),
            ],
        );
    }

    Ok(GoCell {
        root_build_file,
        module_file,
        modules: versions
            .into_iter()
            .map(|(dir, (source_dir, build_file))| ModuleVersion {
                dir: dir.to_owned(),
                source_dir,
                build_file,
            })
            .collect(),
    })
}
