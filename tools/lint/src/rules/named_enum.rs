//! `named-enum`: a script-facing enum's names are written once, by
//! `core::named_enum!` (#1329).
//!
//! A hand-written name table drifts: one copy forgets case-insensitivity, another a
//! variant (#1216). So a top-level `impl X { … }` block of `src/` that holds both a
//! `fn parse(<arg>: &str) -> Option<Self>` (or `Option<X>`) and a
//! `fn name(self) -> &'static str` (or `as_str`, `&self` too) is a finding at the
//! `parse` line: declare the enum through the macro instead. A parse that does more
//! than read one name per variant (aliases with separators ignored, a name built
//! from parts) says why with `// lint: allow(named-enum) <why>`. Test code is not
//! scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "named-enum";

/// Every hand-written `parse` beside a `name` / `as_str` in one impl block.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !path.starts_with("src/") {
        return Vec::new();
    }
    let lines = code_lines(source);
    let mut found = Vec::new();
    let mut block: Option<(String, Option<usize>, bool)> = None;
    for (i, code) in lines.iter().enumerate() {
        if test.get(i).copied().unwrap_or(false) {
            continue;
        }
        if let Some(ty) = impl_type(code) {
            block = Some((ty, None, false));
            continue;
        }
        let Some((ty, parse, name)) = block.as_mut() else {
            continue;
        };
        if code.starts_with('}') {
            if let (Some(at), true) = (*parse, *name) {
                found.push(finding(path, at));
            }
            block = None;
        } else if is_parse(code, ty) {
            *parse = Some(i + 1);
        } else if is_name(code) {
            *name = true;
        }
    }
    found
}

/// `X` of a top-level `impl X {` line (no trait, no generics).
fn impl_type(code: &str) -> Option<String> {
    let ty = code.strip_prefix("impl ")?.strip_suffix('{')?.trim();
    ty.chars()
        .all(|c| c.is_alphanumeric() || c == '_')
        .then(|| ty.to_string())
        .filter(|t| !t.is_empty())
}

fn is_parse(code: &str, ty: &str) -> bool {
    let Some(rest) = code.split("fn parse(").nth(1) else {
        return false;
    };
    let returns = |t: &str| rest.contains(&format!(": &str) -> Option<{t}>"));
    returns("Self") || returns(ty)
}

fn is_name(code: &str) -> bool {
    ["fn name(", "fn as_str("].iter().any(|f| {
        code.split(f).nth(1).is_some_and(|r| {
            r.starts_with("self) -> &'static str") || r.starts_with("&self) -> &'static str")
        })
    })
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
