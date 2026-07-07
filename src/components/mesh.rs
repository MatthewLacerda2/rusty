//! src/components/mesh.rs — Mesh component
//!
//! primitive/FBX vertex+index data. Moved verbatim from the legacy
//! `core/scene.rs`, except the GPU dirty flag is now a `DirtyFlag` newtype: hecs
//! requires every stored component to be `Send + Sync`, and a bare
//! `std::cell::Cell` is `!Sync`. `DirtyFlag` keeps the original interior-mutable
//! `get()`/`set()` API (the renderer clears it while iterating the scene
//! immutably) and is sound to mark `Sync` because the whole engine runs on a
//! single thread.

use crate::asset::{AnimationClip, SkinData};
use crate::render::gpu::mesh::Vertex;
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
    /// How this mesh's geometry is rehydrated on load. `"Box"`/`"Sphere"`/
    /// `"Plane"`/`"Cylinder"` rebuild a primitive; `"Asset"` re-imports an
    /// authored source file (see `asset_ref`). GPU buffers are never persisted.
    pub primitive_type: String,
    /// Path-based reference into an imported source file, `path::sub_object`
    /// (e.g. `project/models/crates.glb::Barrel`). Present iff
    /// `primitive_type == "Asset"`; the scene layer re-imports it on load. This is
    /// the only identity stored — the `.meta` sidecar holds settings, never this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    #[serde(skip)]
    pub vertices: Vec<Vertex>,
    #[serde(skip)]
    pub indices: Vec<u32>,
    /// The bind-pose bone palette for a skinned `"Asset"` mesh, indexed by joint
    /// (matching the vertices' `joint_indices`). Empty for static/primitive meshes.
    /// Like `vertices`, it is never persisted — it is rehydrated from the imported
    /// skin on scene load (#79); the renderer uploads it to the GPU bone buffer.
    #[serde(skip)]
    pub bind_palette: Vec<glam::Mat4>,
    /// The imported skeleton for a skinned `"Asset"` mesh, used by the animation
    /// runtime (#80) to pose the joints. `None` for static/primitive meshes.
    /// Rehydrated from the source on load, never serialized (like `vertices`).
    #[serde(skip)]
    pub skin: Option<SkinData>,
    /// The animation clips that drive this mesh's skeleton, keyed by name from the
    /// source file (#80). Empty for unanimated/static meshes; rehydrated on load.
    #[serde(skip)]
    pub clips: Vec<AnimationClip>,
    /// The currently *posed* bone palette, written each frame by the animation
    /// system (#80) when a clip is driving this skeleton. Empty when no animation is
    /// active; the renderer then falls back to `bind_palette` (the rest pose). Like
    /// the bind palette it is runtime-only and never serialized.
    #[serde(skip)]
    pub pose_palette: Vec<glam::Mat4>,
    // For GPU rendering, we hold the loaded state or buffers in the renderer
    #[serde(skip)]
    pub is_dirty: DirtyFlag, // Set to true when mesh data changes to update GPU buffers
}

impl MeshComponent {
    /// The bone palette the renderer should upload: the live posed palette when an
    /// animation is driving the skeleton, otherwise the static bind pose.
    pub fn active_palette(&self) -> &[glam::Mat4] {
        if self.pose_palette.is_empty() {
            &self.bind_palette
        } else {
            &self.pose_palette
        }
    }

    /// Whether this mesh is skinned (carries an imported skeleton). Frustum culling
    /// leaves skinned meshes uncelled (#330): their cached AABB is the rest pose, which
    /// an animation can push a limb outside of, so testing it risks popping a visible
    /// actor — animated actors are few, while the static geometry that *is* culled is the
    /// bulk of a level.
    pub fn is_skinned(&self) -> bool {
        self.skin.is_some()
    }

    /// World-space AABB of the vertices under `world_matrix`, or `None` for an
    /// empty mesh. (Was `Entity::compute_world_aabb`; lives on the component so
    /// callers reach it through the #344 accessor facade.)
    pub fn world_aabb(&self, world_matrix: glam::Mat4) -> Option<(glam::Vec3, glam::Vec3)> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        for v in &self.vertices {
            let world_pos = world_matrix.transform_point3(glam::Vec3::from_array(v.position));
            min = min.min(world_pos);
            max = max.max(world_pos);
        }
        Some((min, max))
    }
}
