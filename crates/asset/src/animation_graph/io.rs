use std::path::Path;

use super::{AnimationGraph, GraphError};

pub fn load(path: &Path) -> Result<AnimationGraph, GraphError> {
    let text = std::fs::read_to_string(path).map_err(|e| GraphError::Io(e.to_string()))?;
    let mut graph: AnimationGraph =
        serde_json::from_str(&text).map_err(|e| GraphError::Parse(e.to_string()))?;
    graph.validate()?;
    graph.sort_sync_markers();
    Ok(graph)
}

pub fn save(path: &Path, graph: &AnimationGraph) -> Result<(), GraphError> {
    graph.validate()?;
    let json = serde_json::to_string_pretty(graph).map_err(|e| GraphError::Parse(e.to_string()))?;
    // lint: allow(asset-write) an animation graph document, read whole on load
    rusty_core::fs::write_atomic(path, json.as_bytes()).map_err(|e| GraphError::Io(e.to_string()))
}
