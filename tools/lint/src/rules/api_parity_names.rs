//! How `api-parity` finds the ops a file names: by path through its `use` aliases
//! ([`ops_named`]), and, in a module holding its own field-table row, by bare name
//! ([`local_ops_named`]).

use super::{Op, ROOT};
use crate::code::{code_lines, is_ident};
use crate::layers::flatten_use;
use std::collections::{BTreeMap, BTreeSet};

/// Every op `source` names outside its test lines.
pub fn ops_named(source: &str, test: &[bool]) -> BTreeSet<Op> {
    let code = code_lines(source);
    let live: Vec<&str> = (0..code.len())
        .filter(|&i| !test.get(i).copied().unwrap_or(false))
        .map(|i| code[i].as_ref())
        .collect();
    let text = live.join("\n");
    let aliases = aliases(&text);
    let is_op = |path: &str| {
        path.rsplit("::")
            .next()
            .is_some_and(|l| l.starts_with("set_"))
    };
    let bare: BTreeSet<&str> = aliases
        .iter()
        .filter(|(_, path)| is_op(path))
        .map(|(name, _)| name.as_str())
        .collect();
    paths(&text, &bare)
        .into_iter()
        .filter_map(|segs| {
            let (head, rest) = segs.split_first()?;
            let Some((tail, _)) = rest.split_last() else {
                return aliases.get(head).cloned();
            };
            let base = match aliases.get(head.as_str()) {
                Some(base) => base.clone(),
                None if segs.join("::").starts_with(&format!("{ROOT}::")) => {
                    segs[3..segs.len() - 1].join("::")
                }
                None => return None,
            };
            let base = match &segs[..] {
                [first, ..] if first == "crate" => base,
                _ => join(&base, &rest[..rest.len() - 1].join("::")),
            };
            Some(join(&base, tail))
        })
        .collect()
}

/// Every op `source` defines and also calls by its bare name outside its test lines
/// (in its field-table row or knob table), as `module::set_x`.
pub fn local_ops_named(source: &str, test: &[bool], module: &str) -> BTreeSet<Op> {
    let code = code_lines(source);
    let live: Vec<&str> = (0..code.len())
        .filter(|&i| !test.get(i).copied().unwrap_or(false))
        .map(|i| code[i].as_ref())
        .collect();
    let text = live.join("\n");
    let words = |text: &str| -> Vec<(usize, String)> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if !is_ident(chars[i]) || i > 0 && is_ident(chars[i - 1]) {
                i += 1;
                continue;
            }
            let end = (i..chars.len())
                .find(|&j| !is_ident(chars[j]))
                .unwrap_or(chars.len());
            out.push((i, chars[i..end].iter().collect()));
            i = end;
        }
        out
    };
    let chars: Vec<char> = text.chars().collect();
    let before =
        |at: usize, n: usize| -> String { chars[at.saturating_sub(n)..at].iter().collect() };
    let defined: BTreeSet<String> = words(&text)
        .into_iter()
        .filter(|(at, w)| w.starts_with("set_") && before(*at, 3) == "fn ")
        .map(|(_, w)| w)
        .collect();
    words(&text)
        .into_iter()
        .filter(|(at, w)| defined.contains(w) && before(*at, 3) != "fn ")
        .filter(|(at, _)| !matches!(before(*at, 1).as_str(), "." | ":"))
        .map(|(_, w)| join(module, &w))
        .collect()
}

pub(super) fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}::{b}"),
    }
}

/// Each name a `use` of `scene::authoring` binds, to its path under `authoring`
/// (`""` for `authoring` itself, `rigidbody::set_mass` for a setter by name).
fn aliases(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    while let Some(at) = rest.find("use crate::") {
        let after = &rest[at + 4..];
        let end = after.find(';').unwrap_or(after.len());
        for (path, name) in flatten_use(&after[..end]) {
            let path = path.join("::");
            if path == ROOT || path.starts_with(&format!("{ROOT}::")) {
                out.insert(name, path[ROOT.len()..].trim_start_matches("::").into());
            }
        }
        rest = &after[end..];
    }
    out
}

/// Every path in `text` whose last segment is `set_*`, as its segments, of two or
/// more; and every lone `bare` name not called as a method.
fn paths(text: &str, bare: &BTreeSet<&str>) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let starts = is_ident(chars[i]) && (i == 0 || !is_ident(chars[i - 1]));
        let after_colons = i >= 2 && chars[i - 1] == ':' && chars[i - 2] == ':';
        if !starts || after_colons {
            i += 1;
            continue;
        }
        let mut segs = Vec::new();
        let mut j = i;
        loop {
            let from = j;
            while j < chars.len() && is_ident(chars[j]) {
                j += 1;
            }
            segs.push(chars[from..j].iter().collect::<String>());
            let more = chars.get(j) == Some(&':') && chars.get(j + 1) == Some(&':');
            if !(more && chars.get(j + 2).is_some_and(|c| is_ident(*c))) {
                break;
            }
            j += 2;
        }
        let method = i > 0 && chars[i - 1] == '.';
        let lone = segs.len() == 1 && !method && bare.contains(segs[0].as_str());
        if lone || segs.len() >= 2 && segs.last().is_some_and(|s| s.starts_with("set_")) {
            out.push(segs);
        }
        i = j.max(i + 1);
    }
    out
}

#[cfg(test)]
#[path = "api_parity_names_tests.rs"]
mod tests;
