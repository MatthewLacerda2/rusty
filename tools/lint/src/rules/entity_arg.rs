use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "entity-arg";

const NAMES: &[&str] = &[
    "id", "parent", "child", "target", "entity", "body", "canvas",
];

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/script/api/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, _)| !test.get(*i).copied().unwrap_or(false))
        .filter_map(|(i, code)| bare_id(code).map(|name| (i, name)))
        .map(|(i, name)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!(
                "`{name}` is an entity id from Lua: take `api::entity_arg::EntityArg`, so `nil` reads as no entity"
            ),
        })
        .collect()
}

fn bare_id(code: &str) -> Option<String> {
    let bars: Vec<usize> = code.match_indices('|').map(|(at, _)| at).collect();
    bars.windows(2)
        .find_map(|w| callback_id(&code[w[0] + 1..w[1]]))
}

fn callback_id(params: &str) -> Option<String> {
    let parts = split_top(params, ',');
    let (lua, args) = parts.split_first()?;
    let lua = lua.trim();
    if !matches!(lua, "_" | "lua" | "_lua") && !lua.ends_with("&Lua") {
        return None;
    }
    let args = args.join(",");
    let (pat, ty) = args.split_once(':')?;
    let (pats, tys) = (items(pat), items(ty));
    if pats.len() != tys.len() {
        return None;
    }
    pats.into_iter()
        .zip(tys)
        .find(|(p, t)| is_entity(p) && matches!(t.as_str(), "u32" | "Option<u32>"))
        .map(|(p, _)| p)
}

fn items(s: &str) -> Vec<String> {
    let s = s.trim();
    let inner = s.strip_prefix('(').and_then(|s| s.strip_suffix(')'));
    match inner {
        Some(inner) => split_top(inner, ',')
            .iter()
            .map(|x| x.trim().to_string())
            .collect(),
        None => vec![s.to_string()],
    }
}

fn split_top(s: &str, sep: char) -> Vec<&str> {
    let (mut out, mut depth, mut from) = (Vec::new(), 0i32, 0);
    for (at, c) in s.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&s[from..at]);
                from = at + 1;
            }
            _ => {}
        }
    }
    out.push(&s[from..]);
    out
}

fn is_entity(name: &str) -> bool {
    NAMES.contains(&name) || name.ends_with("_id")
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "entity_arg_tests.rs"]
mod tests;
