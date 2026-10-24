//! `sanitize`: every authoring setter refuses a non-finite float (#1361).
//!
//! `scene::authoring::sanitize` holds the float rules the authoring setters share: a
//! value from Lua, the editor or a scene file is finite and in range before it is
//! stored, and NaN or `±inf` takes the field's fallback. Setters kept skipping it
//! (#993, #1341, #1342), so a `pub fn set_*` under `src/scene/authoring/` that takes
//! an `f32` or a glam vector (`Vec2`/`Vec3`/`Vec4`/`Quat`) is a finding unless each
//! such parameter is passed to a `sanitize` helper (a call `helper(param…` of any
//! `fn` that `sanitize.rs` defines) or to another `set_*` of the same file, which this
//! rule checks in turn. A value that is legitimately unbounded and checked elsewhere
//! says so with `// lint: allow(sanitize) <why>`. Test code is not scanned.
//!
//! A line scan, not a parser: the signature runs from `fn set_` to the first `{`,
//! the body to its matching `}`. `param.x` passed alone sanitizes one component, not
//! the parameter, so a helper must take the parameter itself.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, is_ident};

pub const RULE: &str = "sanitize";

/// Where the setters live, and the file of helpers they must use.
const DIR: &str = "src/scene/authoring/";
const HELPERS: &str = "src/scene/authoring/sanitize.rs";

/// Parameter types that can carry a NaN or an infinity.
const FLOATS: &[&str] = &["f32", "Vec2", "Vec3", "Vec4", "Quat"];

/// One `pub fn set_*`: its name, the line it starts on, its float parameters and
/// its body.
struct Setter {
    name: String,
    line: usize,
    floats: Vec<String>,
    body: String,
}

/// Every `fn` name `sanitize.rs` defines outside its tests.
pub fn helpers(source: &str) -> Vec<String> {
    let code = code_lines(source);
    let end = code
        .iter()
        .position(|l| l.contains("#[cfg(test)]"))
        .unwrap_or(code.len());
    code[..end]
        .iter()
        .filter_map(|l| ident_after(l, "fn "))
        .collect()
}

/// Every setter of `source` that stores a float parameter unsanitized.
pub fn check(path: &str, source: &str, test: &[bool], helpers: &[String]) -> Vec<Finding> {
    if !path.starts_with(DIR) || path == HELPERS {
        return Vec::new();
    }
    let code = code_lines(source);
    let code: Vec<&str> = code.iter().map(|c| c.as_ref()).collect();
    let is_test = |i: usize| test.get(i).copied().unwrap_or(false);
    let setters: Vec<Setter> = (0..code.len())
        .filter(|&i| !is_test(i))
        .filter_map(|i| setter(&code, i))
        .collect();
    let names: Vec<&str> = setters.iter().map(|s| s.name.as_str()).collect();
    let mut found = Vec::new();
    for s in &setters {
        let callees = helpers
            .iter()
            .map(String::as_str)
            .chain(names.iter().copied().filter(|n| *n != s.name));
        let callees: Vec<&str> = callees.collect();
        let raw: Vec<&str> = s
            .floats
            .iter()
            .filter(|p| !callees.iter().any(|c| passes(&s.body, c, p)))
            .map(String::as_str)
            .collect();
        if !raw.is_empty() {
            found.push(Finding {
                path: path.to_string(),
                line: s.line,
                rule: RULE,
                message: format!(
                    "`{}` stores `{}` without a `sanitize` helper: NaN and ±inf must take \
                     the field's fallback (scene::authoring::sanitize)",
                    s.name,
                    raw.join("`, `")
                ),
            });
        }
    }
    found
}

/// The setter whose `pub fn set_` is on line `i`, if one is.
fn setter(code: &[&str], i: usize) -> Option<Setter> {
    let line = code[i].trim_start();
    if !line.starts_with("pub") {
        return None;
    }
    let name = ident_after(line, "fn ").filter(|n| n.starts_with("set_"))?;
    let rest: String = code[i..].join("\n");
    let open = rest.find('(')?;
    let close = open + matching(&rest[open..], '(', ')')?;
    let brace = close + rest[close..].find('{')?;
    let end = brace + matching(&rest[brace..], '{', '}')?;
    Some(Setter {
        name,
        line: i + 1,
        floats: float_params(&rest[open + 1..close]),
        body: rest[brace..end].to_string(),
    })
}

/// The names of the parameters in `params` whose type is a float or a glam vector.
fn float_params(params: &str) -> Vec<String> {
    split_top(params)
        .iter()
        .filter_map(|p| p.split_once(':'))
        .filter(|(_, ty)| {
            let ty = ty.trim();
            let ty = ty.rsplit("::").next().unwrap_or(ty);
            FLOATS.contains(&ty)
        })
        .map(|(name, _)| name.trim().trim_start_matches("mut ").trim().to_string())
        .collect()
}

/// `s` split at its top-level commas.
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut from, mut parts) = (0i32, 0, Vec::new());
    for (at, c) in s.char_indices() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[from..at]);
                from = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[from..]);
    parts
}

/// Byte offset of the bracket closing the one `s` starts with.
fn matching(s: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (at, c) in s.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
    }
    None
}

/// Does `body` call `callee` with `param` itself (not one of its fields) among the
/// arguments?
fn passes(body: &str, callee: &str, param: &str) -> bool {
    body.match_indices(callee).any(|(at, _)| {
        let before = body[..at].chars().next_back();
        let after = &body[at + callee.len()..];
        if before.is_some_and(is_ident) || !after.starts_with('(') {
            return false;
        }
        let Some(close) = matching(after, '(', ')') else {
            return false;
        };
        split_top(&after[1..close])
            .iter()
            .any(|arg| arg.trim() == param)
    })
}

/// The identifier right after the first `marker` in `line`.
fn ident_after(line: &str, marker: &str) -> Option<String> {
    let at = line.find(marker)?;
    if line[..at].chars().next_back().is_some_and(is_ident) {
        return None;
    }
    let name: String = line[at + marker.len()..]
        .chars()
        .take_while(|c| is_ident(*c))
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let helpers = helpers(scan.source(HELPERS));
    scan.files
        .iter()
        .flat_map(|p| check(p, scan.source(p), scan.test_lines(p), &helpers))
        .collect()
}

#[cfg(test)]
#[path = "sanitize_tests.rs"]
mod tests;
