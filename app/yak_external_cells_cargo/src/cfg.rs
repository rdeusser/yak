/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Evaluation of the platform conditions of Cargo dependencies, such as
//! `[target.'cfg(unix)'.dependencies]`.

use std::collections::HashSet;

#[derive(yak_error::Error, Debug)]
#[yak(tag = Input)]
enum CfgError {
    #[error("Invalid platform condition `{input}`: {reason}")]
    Invalid { input: String, reason: String },
}

/// CfgExpr is a parsed `cfg(...)` predicate, without the outer `cfg( )`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfgExpr {
    All(Vec<CfgExpr>),
    Any(Vec<CfgExpr>),
    Not(Box<CfgExpr>),
    /// A bare name, such as `unix`.
    Name(String),
    /// A key and a value, such as `target_os = "linux"`.
    KeyValue(String, String),
}

impl CfgExpr {
    pub fn eval(&self, cfg: &TargetCfg) -> bool {
        match self {
            CfgExpr::All(exprs) => exprs.iter().all(|e| e.eval(cfg)),
            CfgExpr::Any(exprs) => exprs.iter().any(|e| e.eval(cfg)),
            CfgExpr::Not(expr) => !expr.eval(cfg),
            CfgExpr::Name(name) => cfg.names.contains(name),
            CfgExpr::KeyValue(key, value) => cfg.key_values.contains(&(key.clone(), value.clone())),
        }
    }
}

/// TargetCfg holds the cfg values of one target, as `rustc --print cfg --target <triple>`
/// prints them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetCfg {
    names: HashSet<String>,
    key_values: HashSet<(String, String)>,
}

impl TargetCfg {
    /// Parses the output of `rustc --print cfg`, one `name` or `key="value"` per line.
    pub fn parse(rustc_output: &str) -> yak_error::Result<TargetCfg> {
        let mut cfg = TargetCfg::default();
        for line in rustc_output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            match CfgExpr::parse(line)? {
                CfgExpr::Name(name) => {
                    cfg.names.insert(name);
                }
                CfgExpr::KeyValue(key, value) => {
                    cfg.key_values.insert((key, value));
                }
                _ => {
                    return Err(CfgError::Invalid {
                        input: line.to_owned(),
                        reason: "expected a name or a key and value".to_owned(),
                    }
                    .into());
                }
            }
        }
        Ok(cfg)
    }
}

/// PlatformCondition is the platform that a Cargo dependency applies to. Cargo writes it as either
/// `cfg(...)` or a target triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformCondition {
    Cfg(CfgExpr),
    Triple(String),
}

impl PlatformCondition {
    pub fn parse(s: &str) -> yak_error::Result<PlatformCondition> {
        let s = s.trim();
        match s
            .strip_prefix("cfg(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            Some(inner) => Ok(PlatformCondition::Cfg(CfgExpr::parse(inner)?)),
            None => Ok(PlatformCondition::Triple(s.to_owned())),
        }
    }

    pub fn matches(&self, triple: &str, cfg: &TargetCfg) -> bool {
        match self {
            PlatformCondition::Cfg(expr) => expr.eval(cfg),
            PlatformCondition::Triple(t) => t == triple,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Str(String),
    LParen,
    RParen,
    Comma,
    Equals,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '=' => tokens.push(Token::Equals),
            '"' => {
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some((_, '"')) => break,
                        Some((_, c)) => value.push(c),
                        None => return Err("unterminated string".to_owned()),
                    }
                }
                tokens.push(Token::Str(value));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        end = i + c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(input[start..end].to_owned()));
            }
            c => return Err(format!("unexpected character `{c}`")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn expect(&mut self, want: Token) -> Result<(), String> {
        match self.next() {
            Some(t) if t == want => Ok(()),
            Some(t) => Err(format!("expected {want:?}, found {t:?}")),
            None => Err(format!("expected {want:?}, found the end")),
        }
    }

    fn expr(&mut self) -> Result<CfgExpr, String> {
        let name = match self.next() {
            Some(Token::Ident(name)) => name,
            Some(t) => return Err(format!("expected a name, found {t:?}")),
            None => return Err("expected a name, found the end".to_owned()),
        };
        match (name.as_str(), self.peek()) {
            ("all" | "any" | "not", Some(Token::LParen)) => {
                self.next();
                let mut args = Vec::new();
                while self.peek() != Some(&Token::RParen) {
                    args.push(self.expr()?);
                    if self.peek() == Some(&Token::Comma) {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RParen)?;
                match name.as_str() {
                    "all" => Ok(CfgExpr::All(args)),
                    "any" => Ok(CfgExpr::Any(args)),
                    _ => match <[CfgExpr; 1]>::try_from(args) {
                        Ok([arg]) => Ok(CfgExpr::Not(Box::new(arg))),
                        Err(args) => Err(format!("not() takes 1 argument, got {}", args.len())),
                    },
                }
            }
            (_, Some(Token::Equals)) => {
                self.next();
                match self.next() {
                    Some(Token::Str(value)) => Ok(CfgExpr::KeyValue(name, value)),
                    Some(t) => Err(format!("expected a string after `{name} =`, found {t:?}")),
                    None => Err(format!("expected a string after `{name} =`")),
                }
            }
            _ => Ok(CfgExpr::Name(name)),
        }
    }
}

impl CfgExpr {
    /// Parses the inside of a `cfg( )` predicate, such as `all(unix, target_arch = "x86_64")`.
    pub fn parse(input: &str) -> yak_error::Result<CfgExpr> {
        let invalid = |reason: String| CfgError::Invalid {
            input: input.to_owned(),
            reason,
        };
        let tokens = tokenize(input).map_err(invalid)?;
        let mut parser = Parser { tokens, pos: 0 };
        let expr = parser.expr().map_err(invalid)?;
        match parser.next() {
            None => Ok(expr),
            Some(t) => Err(invalid(format!("unexpected {t:?} after the expression")).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX_X86_64: &str = r#"
debug_assertions
panic="unwind"
target_arch="x86_64"
target_env="gnu"
target_family="unix"
target_os="linux"
target_pointer_width="64"
unix
"#;

    fn linux() -> TargetCfg {
        TargetCfg::parse(LINUX_X86_64).unwrap()
    }

    fn eval(s: &str) -> bool {
        PlatformCondition::parse(s)
            .unwrap()
            .matches("x86_64-unknown-linux-gnu", &linux())
    }

    #[test]
    fn test_names_and_key_values() {
        assert!(eval("cfg(unix)"));
        assert!(!eval("cfg(windows)"));
        assert!(eval(r#"cfg(target_os = "linux")"#));
        assert!(!eval(r#"cfg(target_os = "macos")"#));
    }

    #[test]
    fn test_combinators() {
        assert!(eval(r#"cfg(all(unix, target_arch = "x86_64"))"#));
        assert!(!eval(r#"cfg(all(unix, target_arch = "aarch64"))"#));
        assert!(eval(r#"cfg(any(windows, target_os = "linux"))"#));
        assert!(eval("cfg(not(windows))"));
        assert!(eval(r#"cfg(all(not(target_env = "msvc"), any(unix,),))"#));
        assert!(eval("cfg(all())"));
        assert!(!eval("cfg(any())"));
    }

    #[test]
    fn test_triples() {
        assert!(eval("x86_64-unknown-linux-gnu"));
        assert!(!eval("aarch64-apple-darwin"));
    }

    #[test]
    fn test_invalid() {
        assert!(PlatformCondition::parse("cfg(all(unix)").is_err());
        assert!(PlatformCondition::parse("cfg(not(unix, windows))").is_err());
        assert!(PlatformCondition::parse(r#"cfg(target_os = linux)"#).is_err());
        assert!(PlatformCondition::parse(r#"cfg(target_os = "linux)"#).is_err());
        assert!(PlatformCondition::parse("cfg(unix windows)").is_err());
    }
}
