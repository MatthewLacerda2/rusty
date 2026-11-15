//! crates/edit/src/authoring/material/graphs.rs — read the graph files a texture recipe
//! instances (#1744), and find the project's and the engine's graphs for
//! `Texture.Ops()`.
//!
//! A graph is a recipe saved as `<name>.graph.json`, named by its path from the
//! project root the way an `image` node names its file, or by an `engine/…` path
//! for one the engine ships (#2185, [`ContentRoots`]). Reading follows nested
//! instances, each file once, by the same rule; [`rusty_procgen::graph::expand`]
//! then refuses a cycle or a nesting too deep, naming the chain.
//!
//! stored-format: the `<name>.graph.json` texture-recipe graph

use std::path::Path;

use rusty_core::naming::variant_case::decode;
use rusty_core::paths::to_slash;
use rusty_core::project::{ContentRoots, ENGINE_PREFIX};
use rusty_procgen::graph::{Graphs, EXTENSION};
use rusty_procgen::TextureRecipe;

/// Every graph `recipe` instances, nested ones included, by the path it is named
/// by. A path without the `.graph.json` extension, or a file that can't be read or
/// parsed, is an error naming the instance and the path.
pub fn read_graphs(recipe: &TextureRecipe, roots: &ContentRoots) -> Result<Graphs, String> {
    let mut graphs = Graphs::new();
    let mut pending: Vec<(String, String)> = owned_paths(recipe);
    while let Some((node, path)) = pending.pop() {
        if graphs.contains_key(&path) {
            continue;
        }
        let graph = read_graph(&path, roots)
            .map_err(|e| format!("graph node '{node}' can't read {path:?}: {e}"))?;
        pending.extend(owned_paths(&graph));
        graphs.insert(path, graph);
    }
    Ok(graphs)
}

/// The graph file `path` names ([`ContentRoots::file`]), parsed as a recipe.
pub fn read_graph(path: &str, roots: &ContentRoots) -> Result<TextureRecipe, String> {
    if !path.ends_with(EXTENSION) {
        return Err(format!("a graph is a {EXTENSION} file"));
    }
    let file = roots.file(path);
    let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    super::sources::warn_if_outside(roots, "graph", path);
    decode(serde_json::from_str(&text).map_err(|e| e.to_string())?)
}

/// Every graph a recipe can instance, as the path it is named by: the project's
/// ([`project_graphs`]), then the engine's ([`engine_graphs`]).
pub fn listed_graphs(roots: &ContentRoots) -> Vec<String> {
    let mut found = project_graphs(&roots.project);
    found.extend(engine_graphs(&roots.engine));
    found
}

/// Every `.graph.json` under `root`, as its path from the root, sorted. Hidden
/// folders and the root's `target/` (a build's output, never a project's assets)
/// are skipped, and so is the root's `engine/`, which an `engine/…` path never
/// reaches (the engine's folder wins). A symlinked folder is not followed.
pub fn project_graphs(root: &Path) -> Vec<String> {
    graphs_under(root, true)
}

/// Every `.graph.json` under the engine's content folder `engine`, as its
/// `engine/…` path, sorted. Only hidden folders are skipped (the project's
/// `target/` and `engine/` rules are its own), and a symlinked folder is not
/// followed.
pub fn engine_graphs(engine: &Path) -> Vec<String> {
    let found = graphs_under(engine, false).into_iter();
    found.map(|path| format!("{ENGINE_PREFIX}{path}")).collect()
}

/// Every `.graph.json` under `root`, sorted; `project` adds the project root's own
/// skips ([`skipped`]).
fn graphs_under(root: &Path, project: bool) -> Vec<String> {
    let mut found = Vec::new();
    walk(root, root, project, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, root: &Path, project: bool, found: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && !skipped(project && dir == root, &name) => {
                walk(&path, root, project, found);
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

/// A hidden folder anywhere, and the project root's own `target/` and `engine/`.
fn skipped(at_root: bool, name: &str) -> bool {
    let engine = ENGINE_PREFIX.trim_end_matches('/');
    name.starts_with('.') || (at_root && (name == "target" || name == engine))
}

fn owned_paths(recipe: &TextureRecipe) -> Vec<(String, String)> {
    let paths = recipe.graph_paths().into_iter();
    paths.map(|(n, p)| (n.to_string(), p.to_string())).collect()
}

#[cfg(test)]
#[path = "graphs_tests.rs"]
mod tests;
