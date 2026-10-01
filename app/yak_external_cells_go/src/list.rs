/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The parts of `go list -e -deps -test -json` output that the cell reads.

use serde::Deserialize;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum ListError {
    #[error("`go list` reported errors:\n{0}")]
    Packages(String),
}

/// Package is one package of `go list -json` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Package {
    pub import_path: String,
    /// The package clause, such as `main`.
    pub name: String,
    /// Whether the package is part of the standard library.
    pub standard: bool,
    /// The package whose tests this variant of a package is compiled for. `go list -test` lists
    /// such variants in addition to the packages themselves.
    pub for_test: Option<String>,
    pub module: Option<Module>,
    pub go_files: Vec<String>,
    pub cgo_files: Vec<String>,
    pub imports: Vec<String>,
    pub test_imports: Vec<String>,
    pub x_test_imports: Vec<String>,
    pub test_go_files: Vec<String>,
    pub x_test_go_files: Vec<String>,
    pub embed_files: Vec<String>,
    pub test_embed_files: Vec<String>,
    pub x_test_embed_files: Vec<String>,
    pub error: Option<PackageError>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Module {
    pub path: String,
    /// Empty for the main module.
    pub version: String,
    /// The absolute path of the directory with the module's files.
    pub dir: String,
    /// Whether the module is the one `go list` ran in.
    pub main: bool,
    /// The module that a `replace` directive puts in this one's place.
    pub replace: Option<Box<Module>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PackageError {
    pub err: String,
}

impl Package {
    /// Whether the package is a test variant: a package that `go list -test` lists again
    /// compiled for a test, the external test package, or the generated test main.
    pub fn is_test_variant(&self) -> bool {
        self.for_test.is_some() || self.import_path.ends_with(".test")
    }

    /// The module that provides the package's files, after any replacement.
    pub fn source_module(&self) -> Option<&Module> {
        let module = self.module.as_ref()?;
        Some(module.replace.as_deref().unwrap_or(module))
    }
}

/// Parses the concatenated JSON objects that `go list -json` prints, and keeps the packages
/// outside the standard library that are not test variants. Fails with the errors of the
/// packages that `go list -e` could not load.
pub fn parse_list(output: &str) -> yak_error::Result<Vec<Package>> {
    let mut packages = Vec::new();
    let mut errors = Vec::new();
    for package in serde_json::Deserializer::from_str(output).into_iter::<Package>() {
        let package = package?;
        if package.standard || package.is_test_variant() {
            continue;
        }
        if let Some(error) = &package.error {
            errors.push(format!("{}: {}", package.import_path, error.err));
        }
        packages.push(package);
    }
    if !errors.is_empty() {
        return Err(ListError::Packages(errors.join("\n")).into());
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variants_and_the_standard_library_are_left_out() {
        let output = r#"
{"ImportPath": "fmt", "Name": "fmt", "Standard": true}
{"ImportPath": "example.com/m/a", "Name": "a", "Module": {"Path": "example.com/m", "Main": true, "Dir": "/m"}, "Imports": ["fmt"], "XTestImports": ["example.com/m/a"]}
{"ImportPath": "example.com/m/a [example.com/m/a.test]", "Name": "a", "ForTest": "example.com/m/a"}
{"ImportPath": "example.com/m/a_test [example.com/m/a.test]", "Name": "a_test", "ForTest": "example.com/m/a"}
{"ImportPath": "example.com/m/a.test", "Name": "main"}
"#;
        let packages = parse_list(output).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].import_path, "example.com/m/a");
        assert!(packages[0].module.as_ref().unwrap().main);
        assert_eq!(packages[0].x_test_imports, ["example.com/m/a"]);
    }

    #[test]
    fn package_errors_fail() {
        let output = r#"{"ImportPath": "example.com/m/a", "Error": {"Err": "no required module provides package x"}}"#;
        let error = parse_list(output).unwrap_err().to_string();
        assert!(
            error.contains("example.com/m/a: no required module provides package x"),
            "{error}"
        );
    }

    #[test]
    fn a_replacement_provides_the_files() {
        let output = r#"{"ImportPath": "example.com/x", "Module": {"Path": "example.com/x", "Version": "v1.0.0", "Replace": {"Path": "example.com/fork", "Version": "v1.1.0", "Dir": "/cache/fork"}}}"#;
        let packages = parse_list(output).unwrap();
        let module = packages[0].source_module().unwrap();
        assert_eq!(
            (
                module.path.as_str(),
                module.version.as_str(),
                module.dir.as_str()
            ),
            ("example.com/fork", "v1.1.0", "/cache/fork")
        );
    }
}
