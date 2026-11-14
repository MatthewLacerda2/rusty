//! The measured module graph: who references whom, from the source.

use super::scan::{self, Ref, Tree};
use crate::modules;
use std::collections::BTreeMap;

/// One edge occurrence: `from` names `rusty_<to>::` at `file:line`.
pub struct Edge {
    pub from: String,
    pub to: String,
    pub at: String,
}

/// The modules: every member crate but the non-module ones (`crate::modules`).
pub fn modules(tree: &Tree) -> Vec<String> {
    modules::all(&tree.root)
}

/// Every cross-module reference in non-test module code.
pub fn edges(tree: &Tree) -> Vec<Edge> {
    let mut out = Vec::new();
    for (file, content) in tree.live_files() {
        let Some(from) = tree.owner(file) else {
            continue;
        };
        let norm = file.to_string_lossy().into_owned();
        for Ref { line, module } in scan::refs(content) {
            if module != from {
                out.push(Edge {
                    from: from.clone(),
                    to: module,
                    at: format!("{norm}:{line}"),
                });
            }
        }
    }
    out
}

/// `from → {to → first occurrence}`, the deduplicated graph.
pub fn summarize(edges: &[Edge]) -> BTreeMap<&str, BTreeMap<&str, &str>> {
    let mut out: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
    for e in edges {
        out.entry(&e.from)
            .or_default()
            .entry(&e.to)
            .or_insert(&e.at);
    }
    out
}
