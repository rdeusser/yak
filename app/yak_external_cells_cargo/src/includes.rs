/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The files that a Rust source file reads at compile time through `include!`, `include_str!`,
//! and `include_bytes!`.

/// IncludedPath is the path argument of an `include!`, `include_str!`, or `include_bytes!` call.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IncludedPath {
    /// A string literal, which names a path relative to the directory of the source file.
    RelativeToSource(String),
    /// `concat!(env!("CARGO_MANIFEST_DIR"), "<path>")`, a path relative to the package's
    /// directory. The path usually starts with `/`.
    RelativeToManifestDir(String),
}

/// IncludedFile is a file that a member's Rust files include from a package other than the
/// member's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IncludedFile {
    /// The file's path relative to the workspace's directory.
    pub path: String,
    /// The directory of the package that owns the file, the nearest directory at or above the file
    /// with a build file, relative to the workspace's directory.
    pub owner: String,
}

const MACROS: &[&str] = &["include_str!", "include_bytes!", "include!"];

/// The paths that `source` includes with a string literal or with
/// `concat!(env!("CARGO_MANIFEST_DIR"), "<path>")`. Other arguments, such as paths in
/// `OUT_DIR`, name files that the build creates. A call in a comment counts as well, which can
/// add a file that the crate does not read.
pub fn included_paths(source: &str) -> Vec<IncludedPath> {
    let mut paths = Vec::new();
    let mut rest = source;
    while let Some((start, len)) = MACROS
        .iter()
        .filter_map(|m| rest.find(m).map(|i| (i, m.len())))
        .min()
    {
        let preceded_by_ident = rest[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after = &rest[start + len..];
        rest = after;
        if preceded_by_ident {
            continue;
        }
        if let Some(path) = call_argument(after) {
            paths.push(path);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// The argument of the macro call that starts at `text`, just after the macro's `!`.
fn call_argument(text: &str) -> Option<IncludedPath> {
    let text = text.trim_start();
    let text = text
        .strip_prefix('(')
        .or_else(|| text.strip_prefix('['))
        .or_else(|| text.strip_prefix('{'))?
        .trim_start();
    if let Some((path, _)) = string_literal(text) {
        return Some(IncludedPath::RelativeToSource(path));
    }
    let text = text
        .strip_prefix("concat!")?
        .trim_start()
        .strip_prefix('(')?;
    let text = text
        .trim_start()
        .strip_prefix("env!")?
        .trim_start()
        .strip_prefix('(')?;
    let (var, text) = string_literal(text.trim_start())?;
    if var != "CARGO_MANIFEST_DIR" {
        return None;
    }
    let text = text
        .trim_start()
        .strip_prefix(')')?
        .trim_start()
        .strip_prefix(',')?;
    let (path, _) = string_literal(text.trim_start())?;
    Some(IncludedPath::RelativeToManifestDir(path))
}

/// The value of the string literal at the start of `text`, a plain or raw string, and the text
/// after it.
fn string_literal(text: &str) -> Option<(String, &str)> {
    if let Some(raw) = text.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let body = raw[hashes..].strip_prefix('"')?;
        let end = format!("\"{}", "#".repeat(hashes));
        let close = body.find(&end)?;
        return Some((body[..close].to_owned(), &body[close + end.len()..]));
    }
    let body = text.strip_prefix('"')?;
    let mut value = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((value, &body[i + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                '\\' => value.push('\\'),
                '"' => value.push('"'),
                '\'' => value.push('\''),
                // An escape that a path does not hold, such as `\u{..}`.
                _ => return None,
            },
            c => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_manifest_dir_paths() {
        let source = r##"
const CONFIG: &str = include_str!("../../rscript/tsconfig.json");
static LOGO: &[u8] = include_bytes!(r#"assets/logo.png"#);
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/generated.rs"));
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
my_include_str!("not/a/call.txt");
const SAME: &str = include_str! ( "../../rscript/tsconfig.json" );
"##;
        assert_eq!(
            included_paths(source),
            [
                IncludedPath::RelativeToSource("../../rscript/tsconfig.json".to_owned()),
                IncludedPath::RelativeToSource("assets/logo.png".to_owned()),
                IncludedPath::RelativeToManifestDir("/src/generated.rs".to_owned()),
            ]
        );
    }

    #[test]
    fn a_body_edit_keeps_the_paths() {
        let before = "fn a() -> &'static str { include_str!(\"a.txt\") }\nfn b() -> u8 { 1 }\n";
        let after = before.replace("1 }", "2 }");
        assert_eq!(included_paths(before), included_paths(&after));
    }
}
