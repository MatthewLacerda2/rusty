//! src/asset/animation_graph/io.rs — load/save the `guard.animgraph` file (#315).
//!
//! The document is pretty JSON, matching the scene file and `.meta` sidecars: the
//! graph is the authorable artifact an agent writes and reviews in a PR, so
//! diffability wins. Both directions validate — a graph never loads with dangling
//! references, and code can't write one either.

use std::path::Path;

use super::{AnimationGraph, GraphError, GRAPH_EXTENSION};

/// True if `path` looks like a graph asset (`guard.animgraph`). Case-insensitive,
/// mirroring [`is_model_path`](crate::asset::is_model_path).
pub fn is_graph_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(GRAPH_EXTENSION))
}

/// Load and validate the graph at `path`. This is what rehydrates the
/// `AnimatorComponent`'s path reference on scene load (#316) — a graph that loads
/// is a graph whose references resolve.
pub fn load(path: &Path) -> Result<AnimationGraph, GraphError> {
    let text = std::fs::read_to_string(path).map_err(|e| GraphError::Io(e.to_string()))?;
    let graph: AnimationGraph =
        serde_json::from_str(&text).map_err(|e| GraphError::Parse(e.to_string()))?;
    graph.validate()?;
    Ok(graph)
}

/// Validate and write `graph` to `path` as pretty (diffable) JSON. Validation
/// first, so an invalid graph can never reach disk from code.
pub fn save(path: &Path, graph: &AnimationGraph) -> Result<(), GraphError> {
    graph.validate()?;
    let json = serde_json::to_string_pretty(graph).map_err(|e| GraphError::Parse(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| GraphError::Io(e.to_string()))
}
