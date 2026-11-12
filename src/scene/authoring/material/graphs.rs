//! src/scene/authoring/material/graphs.rs — read the graph files a texture recipe
//! instances (#1744), and find the project's graphs for `Texture.Ops()`.
//!
//! A graph is a recipe saved as `<name>.graph.json`, named by its path from the
//! project root the way an `image` node names its file. Reading follows nested
//! instances, each file once; [`crate::procgen::graph::expand`] then refuses a
//! cycle or a nesting too deep, naming the chain.
//!
//! stored-format: the `<name>.graph.json` texture-recipe graph

use std::path::Path;

use crate::core::naming::variant_case::decode;
use crate::core::paths::to_slash;
use crate::procgen::graph::{Graphs, EXTENSION};
use crate::procgen::TextureRecipe;

/// Every graph `recipe` instances, nested ones included, by the path it is named
/// by. A path without the `.graph.json` extension, or a file that can't be read or
/// parsed, is an error naming the instance and the path.
pub fn read_graphs(recipe: &TextureRecipe, root: &Path) -> Result<Graphs, String> {
    let mut graphs = Graphs::new();
    let mut pending: Vec<(String, String)> = owned_paths(recipe);
    while let Some((node, path)) = pending.pop() {
        if graphs.contains_key(&path) {
            continue;
        }
        let graph = read_graph(&path, root)
            .map_err(|e| format!("graph node '{node}' can't read {path:?}: {e}"))?;
        pending.extend(owned_paths(&graph));
        graphs.insert(path, graph);
    }
    Ok(graphs)
}

/// The graph file at `path` (from `root`), parsed as a recipe.
pub fn read_graph(path: &str, root: &Path) -> Result<TextureRecipe, String> {
    if !path.ends_with(EXTENSION) {
        return Err(format!("a graph is a {EXTENSION} file"));
    }
    let file = root.join(path);
    let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    super::sources::warn_if_outside(&file, root, "graph", path);
    decode(serde_json::from_str(&text).map_err(|e| e.to_string())?)
}

/// Every `.graph.json` under `root`, as its path from the root, sorted. Hidden
/// folders and the root's `target/` (a build's output, never a project's assets) are skipped,
/// and a symlinked folder is not followed.
pub fn project_graphs(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, root: &Path, found: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && !skipped(dir == root, &name) => {
                walk(&path, root, found);
            }
            Ok(kind) if kind.is_file() && name.ends_with(EXTENSION) => {
                if let Ok(relative) = path.strip_prefix(root) {
                    found.push(to_slash(relative));
                }
            }
            _ => {}
        }
    }
}

/// A hidden folder anywhere, and the project root's own `target/`.
fn skipped(at_root: bool, name: &str) -> bool {
    name.starts_with('.') || (at_root && name == "target")
}

fn owned_paths(recipe: &TextureRecipe) -> Vec<(String, String)> {
    let paths = recipe.graph_paths().into_iter();
    paths.map(|(n, p)| (n.to_string(), p.to_string())).collect()
}

#[cfg(test)]
#[path = "graphs_tests.rs"]
mod tests;
