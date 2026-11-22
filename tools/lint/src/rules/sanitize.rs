use super::finding::Finding;
use super::sanitize_rules::{resolved, unheld, Rule};
use super::scan::Scan;
use crate::code::{code_lines, is_ident};

pub const RULE: &str = "sanitize";

const DIR: &str = "src/edit/authoring/";

const FLOATS: &[&str] = &[
    // sorted: a membership list
    "ColorRange",
    "Curve",
    "EmitShape",
    "f32",
    "f64",
    "Flipbook",
    "Gradient",
    "PhysicsMaterial",
    "Projection",
    "Quat",
    "Range",
    "ReverbParams",
    "UiGradient",
    "Vec2",
    "Vec3",
    "Vec4",
];

struct Setter {
    name: String,
    line: usize,
    floats: Vec<String>,
    body: String,
}

pub fn check(path: &str, source: &str, test: &[bool], rules: &[Rule]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with(DIR) {
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
        let siblings = names.iter().copied().filter(|n| *n != s.name);
        let held = |p: &str| {
            rules
                .iter()
                .any(|r| passes_at(&s.body, &r.name, p, Some(&r.held)))
                || siblings.clone().any(|n| passes_at(&s.body, n, p, None))
        };
        let raw: Vec<&str> = (s.floats.iter())
            .filter(|p| !held(p))
            .map(String::as_str)
            .collect();
        if !raw.is_empty() {
            found.push(Finding {
                path: path.to_string(),
                line: s.line,
                rule: RULE,
                message: format!(
                    "`{}` stores `{}` without a `sanitize` helper: NaN and ±inf must take \
                     the field's fallback (scene::rules)",
                    s.name,
                    raw.join("`, `")
                ),
            });
        }
    }
    found
}

fn setter(code: &[&str], i: usize) -> Option<Setter> {
    let line = code[i].trim_start();
    if !line.starts_with("pub") {
        return None;
    }
    let name = ident_after(line, "fn ").filter(|n| n.starts_with("set_"))?;
    let rest: String = code[i..].join("\n");
    let after_name = rest.find(&format!("fn {name}"))? + 3 + name.len();
    let open = after_name + rest[after_name..].find('(')?;
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

pub(super) fn float_params(params: &str) -> Vec<String> {
    split_top(params)
        .iter()
        .filter_map(|p| p.split_once(':'))
        .filter(|(_, ty)| carries_float(ty))
        .map(|(name, _)| name.trim().trim_start_matches("mut ").trim().to_string())
        .collect()
}

fn carries_float(ty: &str) -> bool {
    let ty = ty
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();
    if let Some(inner) = ty.strip_prefix('[') {
        return carries_float(inner.split([';', ']']).next().unwrap_or(""));
    }
    for wrapper in ["Option<", "Vec<"] {
        if let Some(inner) = ty.strip_prefix(wrapper).and_then(|t| t.strip_suffix('>')) {
            return carries_float(inner);
        }
    }
    FLOATS.contains(&ty.rsplit("::").next().unwrap_or(ty))
}

pub(super) fn split_top(s: &str) -> Vec<&str> {
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

pub(super) fn matching(s: &str, open: char, close: char) -> Option<usize> {
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

pub(super) fn passes_at(
    body: &str,
    callee: &str,
    param: &str,
    positions: Option<&[usize]>,
) -> bool {
    body.match_indices(callee).any(|(at, _)| {
        let before = body[..at].chars().next_back();
        let after = &body[at + callee.len()..];
        if before.is_some_and(is_ident) || !after.starts_with('(') {
            return false;
        }
        let Some(close) = matching(after, '(', ')') else {
            return false;
        };
        let args = split_top(&after[1..close]);
        let at = |k: &usize| args.get(*k).is_some_and(|a| a.trim() == param);
        match positions {
            Some(held) => held.iter().any(at),
            None => args.iter().any(|a| a.trim() == param),
        }
    })
}

pub(super) fn ident_after(line: &str, marker: &str) -> Option<String> {
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

pub fn run(scan: &Scan) -> Vec<Finding> {
    let rules = resolved(scan.files.iter().map(|p| (p.as_str(), scan.source(p))));
    let setters =
        (scan.files.iter()).flat_map(|p| check(p, scan.source(p), scan.test_lines(p), &rules));
    setters.chain(unheld(&rules)).collect()
}

#[cfg(test)]
#[path = "sanitize_tests.rs"]
mod tests;
