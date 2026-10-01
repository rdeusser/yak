/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Writes the Starlark values of the build files that generated external cells serve.

use std::fmt::Write;

/// Value is a Starlark value that a generated build file passes to a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    None,
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Dict(Vec<(String, Value)>),
    /// A Starlark expression, written as is, such as a call of `glob`.
    Expr(String),
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn strs<S: Into<String>>(items: impl IntoIterator<Item = S>) -> Value {
        Value::List(items.into_iter().map(Value::str).collect())
    }

    pub fn render(&self, out: &mut String) {
        match self {
            Value::None => out.push_str("None"),
            Value::Str(s) => quote(s, out),
            Value::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
            Value::Expr(expr) => out.push_str(expr),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.render(out);
                }
                out.push(']');
            }
            Value::Dict(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    quote(key, out);
                    out.push_str(": ");
                    value.render(out);
                }
                out.push('}');
            }
        }
    }
}

/// Writes `s` as a double-quoted Starlark string.
fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Writes a call of `function` with one keyword argument per line.
pub fn call(out: &mut String, function: &str, kwargs: &[(&str, Value)]) {
    out.push_str(function);
    out.push_str("(\n");
    for (name, value) in kwargs {
        out.push_str("    ");
        out.push_str(name);
        out.push_str(" = ");
        value.render(out);
        out.push_str(",\n");
    }
    out.push_str(")\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call() {
        let mut out = String::new();
        call(
            &mut out,
            "rule",
            &[
                ("name", Value::str("a\"b\\c\nd")),
                ("flag", Value::Bool(true)),
                ("deps", Value::strs([":x", ":y"])),
                (
                    "env",
                    Value::Dict(vec![("K".to_owned(), Value::str("v\u{1}"))]),
                ),
            ],
        );
        assert_eq!(
            out,
            "rule(\n    name = \"a\\\"b\\\\c\\nd\",\n    flag = True,\n    deps = [\":x\", \":y\"],\n    env = {\"K\": \"v\\u0001\"},\n)\n\n"
        );
    }
}
