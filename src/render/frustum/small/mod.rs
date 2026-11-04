//! Small-object culling (#1714): a mesh's *Max Draw Distance* (Unreal's) and *Min
//! Screen Size* (Unreal's min screen radius; Unity `LODGroup`'s *Culled* share),
//! tested beside the frustum.
//!
//! The test runs only on what passed the frustum, and only for a mesh that sets one
//! of the two (`MeshComponent::culls_when_small`): per object it is a branch when
//! both are off, a distance and a division when one is on.
//!
//! **Measured from a camera.** A camera's own solids (main view, stacked and
//! render-texture cameras) measure from that camera. The shadow sweeps measure from
//! the **base camera**, not the light, so a prop's shadow goes with the prop and
//! never pops on its own (as `render::lod` picks the casters' level). A bake capture
//! culls nothing ([`SmallObjectCull::off`]): a probe at a prop's far side still sees it.
//!
//! **One size per mesh.** The size is the mesh's local bounds' largest extent times its
//! transform's largest axis scale, as `render::lod` sizes a group (`size · max_scale`)
//! and Unity's `RecalculateBounds` stores one size: rotating or posing the mesh never
//! changes it, so a spinning prop keeps one verdict at one distance. A skinned mesh
//! is sized by its bind-pose box, not its posed bound.

use glam::{Mat4, Vec3};

use crate::components::{MeshComponent, Projection};
use crate::render::lod::{max_scale, relative_height};
use crate::scene::Camera;

/// Where small objects are measured from: a camera's eye and lens, or nowhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SmallObjectCull {
    /// `None`: culls nothing (a bake capture).
    lens: Option<(Vec3, f32, Projection)>,
}

impl SmallObjectCull {
    /// Measure from `cam`: its position, vertical field of view and projection.
    pub(crate) fn from_camera(cam: &Camera) -> Self {
        Self {
            lens: Some((cam.position, cam.fov, cam.projection)),
        }
    }

    /// Cull nothing: what a bake capture draws with.
    pub(crate) fn off() -> Self {
        Self { lens: None }
    }

    /// Whether `mesh` is too far or too small for this view: past its
    /// `max_draw_distance` from the eye to `centre` (its world bounds' centre), or
    /// below its `min_screen_size` ([`relative_height`] at that distance of its size:
    /// `local`'s largest extent times `world`'s largest axis scale). `false` when it
    /// sets neither, or when measured from nowhere. "Too far" is tested first, so a
    /// mesh both culls counts once.
    pub(crate) fn culls(
        &self,
        mesh: &MeshComponent,
        centre: Vec3,
        local: (Vec3, Vec3),
        world: Mat4,
    ) -> bool {
        self.culls_by(SmallLimits::of(mesh), centre, local, world)
    }

    /// [`Self::culls`] for a mesh whose two limits are `limits`, read once and kept
    /// (the retained scene's objects, `render::scene`, #1706).
    pub(crate) fn culls_by(
        &self,
        limits: SmallLimits,
        centre: Vec3,
        (min, max): (Vec3, Vec3),
        world: Mat4,
    ) -> bool {
        let Some((eye, fov, projection)) = self.lens else {
            return false;
        };
        if !limits.any() {
            return false;
        }
        let distance = centre.distance(eye);
        if limits.max_draw_distance > 0.0 && distance > limits.max_draw_distance {
            return true;
        }
        let size = (max - min).max_element() * max_scale(world);
        limits.min_screen_size > 0.0
            && relative_height(size, distance, fov, projection) < limits.min_screen_size
    }
}

/// A mesh's *Max Draw Distance* and *Min Screen Size* (#1714), as a value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SmallLimits {
    pub max_draw_distance: f32,
    pub min_screen_size: f32,
}

impl SmallLimits {
    /// `mesh`'s limits.
    pub(crate) fn of(mesh: &MeshComponent) -> Self {
        Self {
            max_draw_distance: mesh.max_draw_distance,
            min_screen_size: mesh.min_screen_size,
        }
    }

    /// Whether either is set: `MeshComponent::culls_when_small`.
    pub(crate) fn any(self) -> bool {
        self.max_draw_distance > 0.0 || self.min_screen_size > 0.0
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod lod_gpu_tests;
