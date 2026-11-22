//! Existence parity, both directions, between `docs/api/` and the live Lua API surface (signatures are out of scope). The session boots an empty scene so only `api::` namespaces populate `_G`.

use std::collections::{BTreeMap, BTreeSet};

use rusty::dev::session::Session;

fn reference() -> String {
    rusty::script::scripting::api_doc::api_reference().expect("docs/api/ reads")
}

/// Live namespaces deliberately not in the doc; each entry needs a justification.
const NAMESPACE_ALLOWLIST: &[&str] = &[];

type Surface = BTreeMap<String, BTreeSet<String>>;

#[test]
fn doc_matches_live_lua_surface() {
    let sess = Session::new("").expect("session boots in edit mode");
    let live = sess.world().script_manager().api_surface();
    assert!(!live.is_empty(), "the live API surface must not be empty");
    let text = reference();
    let doc = parse_documented_surface(&text);

    let mut problems = Vec::new();
    let names: BTreeSet<&String> = live.keys().chain(doc.keys()).collect();
    for ns in names {
        if NAMESPACE_ALLOWLIST.contains(&ns.as_str()) {
            continue;
        }
        match (live.get(ns), doc.get(ns)) {
            (Some(_), None) => {
                problems.push(format!("namespace registered but not documented: `{ns}`"))
            }
            (None, Some(_)) => {
                problems.push(format!("namespace documented but not registered: `{ns}`"))
            }
            (Some(l), Some(d)) => diff_fns(ns, l, d, &mut problems),
            (None, None) => unreachable!("ns came from one of the two maps"),
        }
    }

    assert!(
        problems.is_empty(),
        "docs/api/ has drifted from the live Lua surface — reconcile the doc \
         (reality is the source of truth for existence):\n  {}",
        problems.join("\n  ")
    );
}

fn diff_fns(ns: &str, live: &BTreeSet<String>, doc: &BTreeSet<String>, out: &mut Vec<String>) {
    let doc_only: Vec<&String> = doc.difference(live).collect();
    let live_only: Vec<&String> = live.difference(doc).collect();
    if !doc_only.is_empty() {
        out.push(format!(
            "[{ns}] documented but not registered: {doc_only:?}"
        ));
    }
    if !live_only.is_empty() {
        out.push(format!(
            "[{ns}] registered but not documented: {live_only:?}"
        ));
    }
}

/// A namespace is an `## ` heading starting with a backticked identifier; a function is a table row whose first cell starts with a backticked `Namespace.Function` (fences skipped).
fn parse_documented_surface(doc: &str) -> Surface {
    let mut surface: Surface = BTreeMap::new();
    let mut in_fence = false;
    for line in doc.lines() {
        let s = line.trim();
        if s.starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence {
            continue;
        } else if let Some(rest) = s.strip_prefix("## ") {
            if let Some(ns) = backticked_ident(rest.trim()) {
                surface.entry(ns).or_default();
            }
        } else if s.starts_with('|') {
            if let Some(first) = s.trim_matches('|').split('|').next() {
                add_table_fn(first.trim(), &mut surface);
            }
        }
    }
    surface
}

fn add_table_fn(cell: &str, surface: &mut Surface) {
    let Some(body) = cell.strip_prefix('`') else {
        return;
    };
    let Some(end) = body.find('`') else { return };
    let Some((ns, first)) = body[..end].split_once('.') else {
        return;
    };
    if !is_ident(ns) || !is_ident(first) {
        return;
    }
    let entry = surface.entry(ns.to_string()).or_default();
    entry.insert(first.to_string());
    if let Some(after) = cell[end + 2..].trim_start().strip_prefix("/ ") {
        if let Some(second) = backticked_ident(after.trim_start()) {
            entry.insert(second);
        }
    }
}

fn backticked_ident(s: &str) -> Option<String> {
    let body = s.strip_prefix('`')?;
    let end = body.find('`')?;
    let ident = &body[..end];
    is_ident(ident).then(|| ident.to_string())
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
