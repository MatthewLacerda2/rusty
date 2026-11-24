//! How far the scene's meshes follow the write log (`core::fs::written`), and the
//! model notes it wrote itself, which `ModelWrites::follow` skips.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct MeshReads {
    /// The write-log generation the meshes hold every note up to.
    pub at: u64,
    /// Model path → the generation of the scene's own re-import note.
    pub reimported: BTreeMap<String, u64>,
}
