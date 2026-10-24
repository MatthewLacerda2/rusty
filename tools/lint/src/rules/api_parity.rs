//! `api-parity`: every inspector op is reachable from Lua (#1368).
//!
//! "Anything the editor does, the API does" (`docs/game-agent.md`). `--parity`
//! holds that the inspector writes through a shared `scene::authoring` op; this rule
//! holds the other half, field by field: every `authoring::<module>::set_*` op that
//! `src/editor/` names is also named from `src/api/`, either called by a named setter
//! or listed in `api::component`'s `op!`/`with!`/`group!` tables (a table entry names
//! the op as a path, which counts the same). An op the inspector alone may write (a
//! display toggle) says why at its `fn` with `// lint: allow(api-parity) <why>`, and
//! an allow on an op that is reached, or that no inspector names, is itself a
//! finding: the rule reads its own allows (`Allow::Own`), so a stale one can't hide.
//!
//! A line scan, not a parser, with known limits:
//! - an op is matched by path: `use crate::scene::authoring::…` trees give each
//!   file's aliases (`rigidbody as rb`, `{self as ops, …}`, `nav::{agent, …}`), and
//!   `alias::…::set_x` or `crate::scene::authoring::…::set_x` names the op;
//! - an op reached only through a helper in another module is not seen as reached;
//! - test code is not scanned.

use super::finding::{allowed, Finding};
use super::scan::Scan;
use crate::code::{code_lines, is_ident};
use std::collections::{BTreeMap, BTreeSet};

pub const RULE: &str = "api-parity";

const AUTHORING: &str = "src/scene/authoring/";
const ROOT: &str = "crate::scene::authoring";
const EDITOR: &str = "src/editor/";
const API: &str = "src/api/";

/// An op by its path under `scene::authoring`: `nav::agent::set_speed`.
type Op = String;

pub fn run(scan: &Scan) -> Vec<Finding> {
    let named = |dir: &str| -> BTreeSet<Op> {
        let files = scan.files.iter().filter(|p| p.starts_with(dir));
        files
            .flat_map(|p| ops_named(scan.source(p), scan.test_lines(p)))
            .collect()
    };
    let ops: BTreeMap<String, &str> = scan
        .files
        .iter()
        .filter(|p| p.starts_with(AUTHORING))
        .map(|p| (p.clone(), scan.source(p)))
        .collect();
    check(&named(EDITOR), &named(API), &ops)
}

/// The findings, given the ops the editor and the API name and the sources of
/// `src/scene/authoring/` by path.
pub fn check(
    editor: &BTreeSet<Op>,
    api: &BTreeSet<Op>,
    authoring: &BTreeMap<String, &str>,
) -> Vec<Finding> {
    let mut found = Vec::new();
    for op in editor.difference(api) {
        let (path, line) = definition(op, authoring).unwrap_or_default();
        if path.is_empty() || !allowed(authoring[&path], line, RULE) {
            found.push(Finding {
                path: if path.is_empty() {
                    AUTHORING.into()
                } else {
                    path
                },
                line: line.max(1),
                rule: RULE,
                message: format!(
                    "the inspector writes through `authoring::{op}` and no Lua binding \
                     reaches it: add an `op!` to `api::component`'s table or a named \
                     setter, or say why it is editor-only (docs/linting.md)"
                ),
            });
        }
    }
    for (path, source) in authoring {
        for (line, name) in allows(source) {
            let op = format!("{}::{name}", module_of(path));
            let why = if !editor.contains(&op) {
                "no inspector writes through it"
            } else if api.contains(&op) {
                "Lua reaches it"
            } else {
                continue;
            };
            found.push(Finding {
                path: path.clone(),
                line,
                rule: RULE,
                message: format!("stale `lint: allow({RULE})` on `{op}`: {why}; remove it"),
            });
        }
    }
    found
}

/// Every op `source` names outside its test lines.
pub fn ops_named(source: &str, test: &[bool]) -> BTreeSet<Op> {
    let code = code_lines(source);
    let live: Vec<&str> = (0..code.len())
        .filter(|&i| !test.get(i).copied().unwrap_or(false))
        .map(|i| code[i].as_ref())
        .collect();
    let text = live.join("\n");
    let aliases = aliases(&text);
    paths(&text)
        .into_iter()
        .filter_map(|segs| {
            let (head, rest) = segs.split_first()?;
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
            Some(join(&base, &segs[segs.len() - 1]))
        })
        .collect()
}

fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}::{b}"),
    }
}

/// Each name a `use` of `scene::authoring` binds, to its path under `authoring`
/// (`""` for `authoring` itself).
fn aliases(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    while let Some(at) = rest.find("use crate::") {
        let after = &rest[at + 4..];
        let end = after.find(';').unwrap_or(after.len());
        let mut leaves = Vec::new();
        use_tree(&after[..end], "", &mut leaves);
        for (path, name) in leaves {
            if path == ROOT || path.starts_with(&format!("{ROOT}::")) {
                out.insert(name, path[ROOT.len()..].trim_start_matches("::").into());
            }
        }
        rest = &after[end..];
    }
    out
}

/// The leaves of a use tree as (full path, the name it binds).
fn use_tree(tree: &str, prefix: &str, out: &mut Vec<(String, String)>) {
    let tree = tree.split_whitespace().collect::<Vec<_>>().join(" ");
    for item in split_top(&tree) {
        let item = item.trim();
        if let Some(open) = item.find('{') {
            let base = item[..open].trim().trim_end_matches("::");
            let inner = item[open + 1..].trim_end().strip_suffix('}').unwrap_or("");
            use_tree(inner, &join_path(prefix, base), out);
            continue;
        }
        let (path, alias) = match item.split_once(" as ") {
            Some((p, a)) => (p.trim(), Some(a.trim())),
            None => (item, None),
        };
        let full = if path == "self" {
            prefix.to_string()
        } else {
            join_path(prefix, path)
        };
        let name = alias.unwrap_or_else(|| full.rsplit("::").next().unwrap_or(""));
        if !name.is_empty() && name != "_" {
            out.push((full.clone(), name.to_string()));
        }
    }
}

fn join_path(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}::{b}")
    }
}

/// `s` split at its top-level commas.
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut from, mut parts) = (0i32, 0, Vec::new());
    for (at, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[from..at]);
                from = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[from..]);
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

/// Every path in `text` whose last segment is `set_*`, as its segments, of two or
/// more.
fn paths(text: &str) -> Vec<Vec<String>> {
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
        if segs.len() >= 2 && segs.last().is_some_and(|s| s.starts_with("set_")) {
            out.push(segs);
        }
        i = j.max(i + 1);
    }
    out
}

/// `src/scene/authoring/nav/agent.rs` → `nav::agent`; a `mod.rs` is its folder.
fn module_of(path: &str) -> String {
    let rel = path.trim_start_matches(AUTHORING).trim_end_matches(".rs");
    let rel = rel.trim_end_matches("/mod").trim_end_matches("mod");
    rel.trim_end_matches('/').replace('/', "::")
}

/// Where `op`'s `fn` is defined: in its module's file, or a file of its folder
/// that the module re-exports.
fn definition(op: &str, authoring: &BTreeMap<String, &str>) -> Option<(String, usize)> {
    let (module, name) = op.rsplit_once("::").unwrap_or(("", op));
    let dir = format!("{AUTHORING}{}", module.replace("::", "/"));
    let candidates = authoring.iter().filter(|(p, _)| {
        module_of(p) == module
            || p.strip_prefix(&format!("{dir}/"))
                .is_some_and(|f| !f.contains('/'))
    });
    let needle = format!("fn {name}(");
    candidates.into_iter().find_map(|(p, src)| {
        let at = src.lines().position(|l| l.contains(&needle))?;
        Some((p.clone(), at + 1))
    })
}

/// Each `lint: allow(api-parity)` in `source`, with the `set_*` fn it sits on.
fn allows(source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let marker = format!("lint: allow({RULE})");
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !l.contains(&marker) || !l.trim_start().starts_with("//") {
            continue;
        }
        let item = lines[i..].iter().find(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with("#[")
        });
        let name = item.and_then(|l| {
            let at = l.find("fn set_")? + 3;
            Some(l[at..].chars().take_while(|c| is_ident(*c)).collect())
        });
        out.push((i + 1, name.unwrap_or_default()));
    }
    out
}

#[cfg(test)]
#[path = "api_parity_tests.rs"]
mod tests;
