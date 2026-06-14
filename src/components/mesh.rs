//! src/components/mesh.rs — Mesh component
//!
//! primitive/FBX vertex+index data. Moved verbatim from the legacy
//! `core/scene.rs`, except the GPU dirty flag is now a `DirtyFlag` newtype: hecs
//! requires every stored component to be `Send + Sync`, and a bare
//! `std::cell::Cell` is `!Sync`. `DirtyFlag` keeps the original interior-mutable
//! `get()`/`set()` API (the renderer clears it while iterating the scene
//! immutably) and is sound to mark `Sync` because the whole engine runs on a
//! single thread.

use crate::render::mesh::Vertex;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

/// Interior-mutable boolean that is `Sync` (single-threaded engine invariant).
#[derive(Debug, Default)]
pub struct DirtyFlag(Cell<bool>);

// SAFETY: the engine is single-threaded; the renderer is the only place that
// mutates this flag, and it never crosses a thread boundary. hecs only needs the
// `Sync` bound for its (unused here) parallel-iteration APIs.
unsafe impl Sync for DirtyFlag {}

impl DirtyFlag {
    pub fn new(value: bool) -> Self {
        Self(Cell::new(value))
    }

    pub fn get(&self) -> bool {
        self.0.get()
    }

    pub fn set(&self, value: bool) {
        self.0.set(value);
    }
}

impl Clone for DirtyFlag {
    fn clone(&self) -> Self {
        Self::new(self.0.get())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshComponent {
    pub primitive_type: String, // "Box", "Sphere", "Plane", "Cylinder", "FBX"
    #[serde(skip)]
    pub vertices: Vec<Vertex>,
    #[serde(skip)]
    pub indices: Vec<u32>,
    // For GPU rendering, we hold the loaded state or buffers in the renderer
    #[serde(skip)]
    pub is_dirty: DirtyFlag, // Set to true when mesh data changes to update GPU buffers
}
