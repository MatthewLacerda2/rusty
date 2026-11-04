//! What one camera draws of the scene's meshes, before LOD and culling: its culling
//! mask, the Static filter a capture applies, the world bounds the frustum and the
//! small-object cull test (#330, #1714), and where that cull measures from.

use glam::Vec3;

use crate::render::lod::LodSelection;
use crate::render::{Frustum, MeshId, Renderer, SmallObjectCull};
use crate::scene::Scene;

/// A mesh's world bounds, `None` when they cannot be known (never culled).
type Bounds = Option<(Vec3, Vec3)>;

/// What one camera's culling skipped, by reason (#433's counters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CameraCulls {
    /// Mesh entities tested against the frustum (#1714): every one the camera draws
    /// at all whose LOD level it shows.
    pub tested: u32,
    /// Outside the frustum (#330).
    pub culled: u32,
    /// Inside it, but too far or too small (#1714).
    pub size_culled: u32,
    /// Hidden because their LOD level is not the one shown (#472).
    pub lod_hidden: u32,
}

/// One camera's culling inputs: its culling mask (#92), the LOD levels it shows, its
/// frustum and its small-object cull.
pub(super) struct ViewCull<'a> {
    pub mask: u32,
    pub lod: &'a LodSelection,
    pub frustum: &'a Frustum,
    pub small: SmallObjectCull,
}

impl Renderer {
    /// Mesh entity `id`'s world bounds when `view` draws it, its verdict counted in
    /// `culls` when it doesn't: its mask or a capture leaves it out, then its LOD
    /// level is hidden (#472), then the frustum (#330), then too far or too small
    /// on what the frustum kept (#1714). Whichever culls first counts.
    pub(super) fn visible_bounds(
        &self,
        scene: &Scene,
        id: u32,
        view: &ViewCull,
        culls: &mut CameraCulls,
    ) -> Option<Bounds> {
        if !self.camera_draws(scene, id, view.mask) {
            return None;
        }
        if view.lod.hides(id) {
            culls.lod_hidden += 1;
            return None;
        }
        culls.tested += 1;
        let bounds = self.world_bounds(scene, id);
        if bounds.is_some_and(|(min, max)| !view.frustum.intersects_aabb(min, max)) {
            culls.culled += 1;
            return None;
        }
        if bounds.is_some_and(|b| self.too_small(scene, id, view.small, b)) {
            culls.size_culled += 1;
            return None;
        }
        Some(bounds)
    }

    /// Whether a camera whose culling mask (#92) is `mask` draws mesh entity `id` at
    /// all, before LOD and frustum culling.
    fn camera_draws(&self, scene: &Scene, id: u32, mask: u32) -> bool {
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

    /// Whether `small` culls mesh entity `id`, whose world bounds are `(min, max)`:
    /// measured to their centre, sized by its mesh's local box (#1714).
    fn too_small(
        &self,
        scene: &Scene,
        id: u32,
        small: SmallObjectCull,
        (min, max): (Vec3, Vec3),
    ) -> bool {
        let Some(mesh) = scene.world.mesh(id) else {
            return false;
        };
        if !mesh.culls_when_small() {
            return false;
        }
        let Some(gpu_mesh) = self.gpu_meshes.get(&MeshId::from_mesh(&mesh)) else {
            return false;
        };
        let world = scene.world.local_to_world(id);
        small.culls(&mesh, (min + max) * 0.5, gpu_mesh.local_aabb, world)
    }

    /// The entity's world bounds `(min, max)`: its mesh's cached local AABB under the
    /// entity's world matrix, or its animated bound when skinned (#833). The frustum
    /// cull (#330) tests them and the draw picks its reflection probe at their centre
    /// (#1206). `None` for an entity with no mesh, whose geometry is not resident on
    /// the GPU yet, or whose pose cannot be bounded: it is never culled, since the
    /// downstream sync produces no draw for a non-resident mesh anyway, and culling a
    /// not-yet-uploaded mesh could wrongly hide it on its first frame.
    fn world_bounds(&self, scene: &Scene, id: u32) -> Option<(glam::Vec3, glam::Vec3)> {
        let mesh = scene.world.mesh(id)?;
        let gpu_mesh = self.gpu_meshes.get(&MeshId::from_mesh(&mesh))?;
        gpu_mesh.world_bounds(&mesh, scene.world.local_to_world(id))
    }

    /// Where `cam`'s small-object cull measures from (#1714): `cam` itself, or
    /// nowhere during a capture, whose bake sees every prop whatever its size.
    pub(crate) fn small_object_cull(&self, cam: &crate::scene::Camera) -> SmallObjectCull {
        match self.capture.static_only {
            true => SmallObjectCull::off(),
            false => SmallObjectCull::from_camera(cam),
        }
    }
}
