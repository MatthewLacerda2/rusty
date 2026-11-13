//! The measured module graph: who references whom, from the source.

use super::scan::{self, Ref, Tree};
use crate::modules;
use std::collections::BTreeMap;
use std::fs;

/// One edge occurrence: `from` names `crate::<to>` at `file:line`.
pub struct Edge {
    pub from: String,
    pub to: String,
    pub at: String,
}

/// The top-level modules: every directory under `src` but `bin`, plus any
/// `src/<name>.rs` that is not a crate root, plus every moved module's crate.
pub fn modules(tree: &Tree) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(tree.root.join("src"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if tree.is_test_only(&path) {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let is_mod = if path.is_dir() {
                name != "bin"
            } else {
                path.extension().is_some_and(|x| x == "rs") && !matches!(&*name, "lib" | "main")
            };
            is_mod.then_some(name)
        })
        .chain(modules::moved(&tree.root))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every cross-module reference in non-test module code.
pub fn edges(tree: &Tree) -> Vec<Edge> {
    let mut out = Vec::new();
    for (file, content) in tree.live_files() {
        let Some(from) = tree.owner(file) else {
            continue;
        };
        let norm = file.to_string_lossy().into_owned();
        let in_crate = tree.crate_of(file).is_some();
        for Ref { line, module } in scan::refs(content, in_crate) {
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
