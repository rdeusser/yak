/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The parts of a Go file that can change the output of `go list`.

/// The parts of a Go source file that can change what `go list` reports for its package: the
/// text before the first declaration, which holds the build constraints, the package clause,
/// the imports, and a cgo preamble, and every `//go:embed` line. An edit outside them, such as
/// one to a function body, leaves the result unchanged.
///
/// Go allows imports only before the first declaration, and a declaration at the top level
/// starts a line with its keyword, so the header ends at the first line that does. The result
/// only needs to change whenever `go list` output can, so a line that only looks like a
/// declaration, such as one inside a raw string, ends the header early and adds nothing.
pub fn go_file_header(source: &str) -> String {
    let mut header = String::new();
    let mut in_header = true;
    for line in source.lines() {
        if in_header && is_declaration(line) {
            in_header = false;
        }
        if in_header || line.trim_start().starts_with("//go:embed") {
            header.push_str(line);
            header.push('\n');
        }
    }
    header
}

fn is_declaration(line: &str) -> bool {
    ["func", "type", "var", "const"].iter().any(|keyword| {
        line.strip_prefix(keyword)
            .is_some_and(|rest| rest.starts_with([' ', '\t', '(']))
    })
}

/// Reports whether a change to `path`, a `/`-separated path inside a Go module's directory, can
/// change the files of the module's go cell. `added_or_removed` says whether the file appeared
/// or disappeared, as opposed to changing its contents, and `build_file_names` are the names of
/// build files in the module's cell.
///
/// The cell reads `go.mod`, `go.sum`, the header of each Go file, which build files call
/// `go_package()`, and the names of the module's files, which decide its packages and the files
/// that `//go:embed` patterns match.
pub fn is_cell_input(path: &str, added_or_removed: bool, build_file_names: &[&str]) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    added_or_removed
        || matches!(name, "go.mod" | "go.sum")
        || name.ends_with(".go")
        || build_file_names.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"//go:build linux

// Package a reads files.
package a

/*
#include <stdio.h>
*/
import "C"

import (
	_ "embed"
	"fmt"
)

//go:embed data.txt
var data string

func Print() {
	fmt.Println(data)
}
"#;

    #[test]
    fn the_header_ends_at_the_first_declaration() {
        assert_eq!(
            go_file_header(SOURCE),
            "//go:build linux\n\n// Package a reads files.\npackage a\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n\nimport (\n\t_ \"embed\"\n\t\"fmt\"\n)\n\n//go:embed data.txt\n"
        );
    }

    #[test]
    fn a_function_body_is_not_in_the_header() {
        let edited = SOURCE.replace("fmt.Println(data)", "fmt.Print(data)");
        assert_eq!(go_file_header(SOURCE), go_file_header(&edited));
    }

    #[test]
    fn imports_and_embeds_are_in_the_header() {
        for edited in [
            SOURCE.replace("\"fmt\"", "\"os\""),
            SOURCE.replace("//go:embed data.txt", "//go:embed other.txt"),
            SOURCE.replace("//go:build linux", "//go:build darwin"),
        ] {
            assert_ne!(go_file_header(SOURCE), go_file_header(&edited));
        }
    }

    #[test]
    fn grouped_declarations_end_the_header() {
        assert!(is_declaration("var ("));
        assert!(is_declaration("const(x = 1)"));
        assert!(!is_declaration("variable := 1"));
        assert!(!is_declaration("\tvar x int"));
    }

    #[test]
    fn cell_inputs() {
        for path in ["go.mod", "api/go.sum", "api/server.go", "api/YAK"] {
            assert!(is_cell_input(path, false, &["YAK"]), "{path}");
        }
        for path in ["go.mod.bak", "docs/go.md", "api/data.json"] {
            assert!(!is_cell_input(path, false, &["YAK"]), "{path}");
        }
        assert!(is_cell_input("api/data.json", true, &["YAK"]));
    }
}
