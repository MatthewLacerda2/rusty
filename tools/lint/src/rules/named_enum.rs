//! `named-enum`: a script-facing enum's names are written once, by
//! `core::named_enum!` (#1329).
//!
//! A hand-written name table drifts: one copy forgets case-insensitivity, another a
//! variant (#1216). So a file of `src/` that writes, for one type `T`, both
//!
//! - a parse — `fn parse(_: &str) -> Option<Self>` (or `Option<T>`) in an
//!   `impl T` block, or a free `fn parse_*(_: &str) -> Option<T>` — and
//! - a name — `fn name` / `fn as_str` taking `self` (or `&self`) and returning
//!   `&'static str` in an `impl T` block, or a free `fn *_name(_: T) -> &'static str`
//!
//! is a finding at the parse: declare `T` through the macro instead. The two may sit
//! in different `impl T` blocks, an `impl` may be indented (an inline `mod`) and
//! carry a comment after its `{`. So is a `const` / `static` table of
//! `(T, &str)` pairs (`[(T, &str); N]`, `&[(T, &'static str)]`), a name table read
//! both ways by a generic lookup, at its declaration. A parse that does more than read one name per
//! variant (separators ignored, a name built from parts) says why with
//! `// lint: allow(named-enum) <why>`. Test code is not scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_of, Open};
use std::collections::BTreeMap;

pub const RULE: &str = "named-enum";

/// Every hand-written parse of a type that also has a hand-written name.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !path.starts_with("src/") {
        return Vec::new();
    }
    // Per type: the line of its first parse, and whether it has a name.
    let mut types: BTreeMap<String, (Option<usize>, bool)> = BTreeMap::new();
    let (mut open, mut depth) = (Open::None, 0usize);
    let mut block: Option<(String, usize)> = None;
    let mut tables = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let mut braces = (0usize, 0usize);
        let code = code_of(line, &mut open, |_, c| match c {
            '{' => braces.0 += 1,
            '}' => braces.1 += 1,
            _ => {}
        });
        let code = code.trim();
        if !test.get(i).copied().unwrap_or(false) {
            if let Some(ty) = block.as_ref().map(|b| b.0.as_str()) {
                if is_method_parse(code, ty) {
                    types
                        .entry(ty.to_string())
                        .or_default()
                        .0
                        .get_or_insert(i + 1);
                } else if is_method_name(code) {
                    types.entry(ty.to_string()).or_default().1 = true;
                }
            }
            if let Some(ty) = free_parse(code) {
                types.entry(ty).or_default().0.get_or_insert(i + 1);
            } else if let Some(ty) = free_name(code) {
                types.entry(ty).or_default().1 = true;
            }
            if block.is_none() {
                block = impl_type(code).map(|ty| (ty, depth));
            }
            if is_pair_table(code) {
                tables.push(i + 1);
            }
        }
        depth = (depth + braces.0).saturating_sub(braces.1);
        if block.as_ref().is_some_and(|b| depth <= b.1) && braces.1 > 0 {
            block = None;
        }
    }
    let mut lines: Vec<usize> = types
        .into_values()
        .filter_map(|(parse, name)| parse.filter(|_| name))
        .chain(tables)
        .collect();
    lines.sort_unstable();
    lines.into_iter().map(|line| finding(path, line)).collect()
}

/// A `const` / `static` whose type is an array or slice of `(T, &str)` pairs.
fn is_pair_table(code: &str) -> bool {
    let decl = ["const ", "static "].iter().find_map(|k| {
        code.split_once(k)
            .filter(|(before, _)| is_visibility(before))
    });
    let Some((_, rest)) = decl else {
        return false;
    };
    let Some(ty) = rest
        .split_once(':')
        .and_then(|(_, t)| t.split_once('='))
        .map(|t| t.0)
    else {
        return false;
    };
    let ty = ty
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("'static")
        .trim();
    let Some(pair) = ty
        .strip_prefix("[(")
        .and_then(|t| t.split_once(')'))
        .map(|t| t.0)
    else {
        return false;
    };
    let parts: Vec<&str> = pair.split(',').map(str::trim).collect();
    matches!(parts[..], [t, "&str" | "&'static str"] if is_type(t))
}

/// Whether `before` (the text before `const` / `static`) is only a visibility.
fn is_visibility(before: &str) -> bool {
    let b = before.trim();
    b.is_empty() || b == "pub" || (b.starts_with("pub(") && b.ends_with(')'))
}

/// `T` of an `impl T {` line (no trait, no generics), a comment after it cut.
fn impl_type(code: &str) -> Option<String> {
    let ty = code.strip_prefix("impl ")?.strip_suffix('{')?.trim();
    is_type(ty).then(|| ty.to_string())
}

fn is_type(ty: &str) -> bool {
    !ty.is_empty() && ty.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn is_method_parse(code: &str, ty: &str) -> bool {
    let Some(rest) = code.split("fn parse(").nth(1) else {
        return false;
    };
    returns_option(rest).is_some_and(|t| t == "Self" || t == ty)
}

fn is_method_name(code: &str) -> bool {
    ["fn name(", "fn as_str("].iter().any(|f| {
        code.split(f).nth(1).is_some_and(|r| {
            r.starts_with("self) -> &'static str") || r.starts_with("&self) -> &'static str")
        })
    })
}

/// `T` of a free `fn parse_*(<arg>: &str) -> Option<T>`.
fn free_parse(code: &str) -> Option<String> {
    let rest = code.split("fn parse_").nth(1)?;
    let rest = &rest[rest.find('(')?..];
    returns_option(rest)
        .filter(|t| *t != "Self")
        .map(str::to_string)
}

/// `T` of a free `fn *_name(<arg>: T) -> &'static str`.
fn free_name(code: &str) -> Option<String> {
    let at = code.find("fn ")?;
    let sig = &code[at + 3..];
    let open = sig.find('(')?;
    if !sig[..open].ends_with("_name") {
        return None;
    }
    let (arg, rest) = sig[open + 1..].split_once(')')?;
    let ty = arg.split_once(':')?.1.trim();
    (is_type(ty) && rest.trim_start().starts_with("-> &'static str")).then(|| ty.to_string())
}

/// `T` when `rest` (from just after a parse's `(`, or at it) is a single `&str`
/// argument returning `Option<T>`.
fn returns_option(rest: &str) -> Option<&str> {
    let rest = rest.strip_prefix('(').unwrap_or(rest);
    let (arg, ret) = rest.split_once(')')?;
    if arg.split_once(':')?.1.trim() != "&str" {
        return None;
    }
    let ty = ret
        .trim_start()
        .strip_prefix("-> Option<")?
        .split_once('>')?
        .0;
    is_type(ty).then_some(ty)
}

fn finding(path: &str, line: usize) -> Finding {
    Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message:
            "a hand-written name table: declare the enum with `crate::core::named_enum!` (#1329)"
                .to_string(),
    }
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|path, source| check(path, source, scan.test_lines(path)))
}

#[cfg(test)]
#[path = "named_enum_tests.rs"]
mod tests;
