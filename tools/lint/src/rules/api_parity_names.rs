use super::{Op, ROOT};
use crate::code::{code_lines, is_ident};
use crate::layers::flatten_use;
use crate::modules::{crate_module, module_path};
use std::collections::{BTreeMap, BTreeSet};

pub fn ops_named(source: &str, test: &[bool], root: &str) -> BTreeSet<Op> {
    let code = code_lines(source);
    let live: Vec<&str> = (0..code.len())
        .filter(|&i| !test.get(i).copied().unwrap_or(false))
        .map(|i| code[i].as_ref())
        .collect();
    let text = live.join("\n");
    let aliases = aliases(&text, root);
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
                Some(base) => join(base, &rest[..rest.len() - 1].join("::")),
                None if segs.join("::").starts_with(&format!("{root}::")) => {
                    segs[root.split("::").count()..segs.len() - 1].join("::")
                }
                None => return None,
            };
            Some(join(&base, tail))
        })
        .collect()
}

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

pub fn root_of(path: &str) -> &'static str {
    let module = module_path(path);
    let module = module
        .strip_prefix("src/")
        .and_then(|p| p.split('/').next());
    match (crate_module(path).is_some(), module) {
        (true, Some("edit")) => "crate::authoring",
        (true, _) => "rusty_edit::authoring",
        (false, _) => ROOT,
    }
}

pub(super) fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}::{b}"),
    }
}

fn aliases(text: &str, root: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let first = root.split("::").next().unwrap_or(root);
    let needle = format!("use {first}::");
    let mut rest = text;
    while let Some(at) = rest.find(&needle) {
        let after = &rest[at + 4..];
        let end = after.find(';').unwrap_or(after.len());
        for (path, name) in flatten_use(&after[..end]) {
            let path = path.join("::");
            if path == root || path.starts_with(&format!("{root}::")) {
                out.insert(name, path[root.len()..].trim_start_matches("::").into());
            }
        }
        rest = &after[end..];
    }
    out
}

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
