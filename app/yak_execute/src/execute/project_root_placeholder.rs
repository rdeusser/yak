/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The placeholder that stands for the project root in the action digest of a command whose
//! arguments and environment name paths by absolute path, so that two checkouts of a project at
//! different paths compute the same digest for the same command.

use std::borrow::Cow;

/// PROJECT_ROOT_PLACEHOLDER is the text that replaces the project root in the command that an
/// action digest covers.
pub const PROJECT_ROOT_PLACEHOLDER: &str = "${YAK_PROJECT_ROOT}";

/// replace_project_root returns `value` with `PROJECT_ROOT_PLACEHOLDER` in place of each
/// occurrence of `root` that is a whole path or the start of one, such as `/repo` in
/// `--dir=/repo/src`. It leaves `root` in place where it continues a longer name, as in
/// `/repository` or `/other/repo`.
pub(crate) fn replace_project_root<'a>(value: &'a str, root: &str) -> Cow<'a, str> {
    if root.is_empty() || !value.contains(root) {
        return Cow::Borrowed(value);
    }
    let mut replaced = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find(root) {
        let before = rest[..start].chars().next_back();
        let after = &rest[start + root.len()..];
        let starts_path = !before.is_some_and(continues_path);
        let ends_component = after.is_empty() || after.starts_with(['/', '\\']);
        replaced.push_str(&rest[..start]);
        if starts_path && ends_component {
            replaced.push_str(PROJECT_ROOT_PLACEHOLDER);
        } else {
            replaced.push_str(root);
        }
        rest = after;
    }
    replaced.push_str(rest);
    Cow::Owned(replaced)
}

/// Whether `c`, before an absolute path, makes that path the rest of a longer one.
fn continues_path(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '/' | '\\' | '.' | '_' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_root_where_a_path_starts() {
        let root = "/home/me/repo";
        assert_eq!(
            replace_project_root("/home/me/repo/yak-out/test", root),
            "${YAK_PROJECT_ROOT}/yak-out/test"
        );
        assert_eq!(
            replace_project_root("/home/me/repo", root),
            "${YAK_PROJECT_ROOT}"
        );
        assert_eq!(
            replace_project_root("--dir=/home/me/repo/a:/home/me/repo/b", root),
            "--dir=${YAK_PROJECT_ROOT}/a:${YAK_PROJECT_ROOT}/b"
        );
        assert_eq!(
            replace_project_root(r"C:\repo\yak-out", r"C:\repo"),
            r"${YAK_PROJECT_ROOT}\yak-out"
        );
    }

    #[test]
    fn keeps_the_root_inside_another_name() {
        let root = "/home/me/repo";
        assert_eq!(
            replace_project_root("/home/me/repository/a", root),
            "/home/me/repository/a"
        );
        assert_eq!(
            replace_project_root("/mnt/home/me/repo/a", root),
            "/mnt/home/me/repo/a"
        );
        assert!(matches!(
            replace_project_root("--verbose", root),
            Cow::Borrowed("--verbose")
        ));
    }
}
