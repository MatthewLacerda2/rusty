//! What one camera draws of the scene's meshes, before LOD and culling: its culling
//! mask, the Static filter a capture applies, the world bounds the frustum and the
//! small-object cull test (#330, #1714), and where that cull measures from.

use crate::render::{MeshId, Renderer, SmallObjectCull};
use crate::scene::Scene;

impl Renderer {
    /// Whether a camera whose culling mask (#92) is `mask` draws mesh entity `id` at
    /// all, before LOD and frustum culling.
    pub(super) fn camera_draws(&self, scene: &Scene, id: u32, mask: u32) -> bool {
        // An inactive mesh, or a shadows-only one: it casts, no camera draws it (#1571).
        let mode = scene.world.mesh(id).map(|m| m.cast_shadows);
        if !scene.world.is_active_in_hierarchy(id) || mode.is_some_and(|m| !m.is_visible()) {
            return false;
        }
        // During a static-cubemap capture, gather only static geometry — dynamic
        // actors must not bake into a probe/reflection (#243), as the static shadow
        // sweeps take static casters only (`passes::shadows::casters::sweep`).
        if self.capture.static_only && !scene.world.is_static(id) {
            return false;
        }
        crate::scene::layer_in_mask(scene.world.layer(id), mask)
    }

    /// The entity's world bounds `(min, max)`: its mesh's cached local AABB under the
    /// entity's world matrix, or its animated bound when skinned (#833). The frustum
    /// cull (#330) tests them and the draw picks its reflection probe at their centre
    /// (#1206). `None` for an entity with no mesh, whose geometry is not resident on
    /// the GPU yet, or whose pose cannot be bounded: it is never culled, since the
    /// downstream sync produces no draw for a non-resident mesh anyway, and culling a
    /// not-yet-uploaded mesh could wrongly hide it on its first frame.
    pub(super) fn world_bounds(&self, scene: &Scene, id: u32) -> Option<(glam::Vec3, glam::Vec3)> {
        let mesh = scene.world.mesh(id)?;
        let gpu_mesh = self.gpu_meshes.get(&MeshId::from_mesh(&mesh))?;
        gpu_mesh.world_bounds(&mesh, scene.world.local_to_world(id))
    }

    /// Where `cam`'s small-object cull measures from (#1714): `cam` itself, or
    /// nowhere during a capture, whose bake sees every prop whatever its size.
    pub(super) fn small_object_cull(&self, cam: &crate::scene::Camera) -> SmallObjectCull {
        match self.capture.static_only {
            true => SmallObjectCull::off(),
            false => SmallObjectCull::from_camera(cam),
        }
    }
}
