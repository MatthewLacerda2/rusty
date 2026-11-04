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

use glam::Vec3;

use crate::components::{MeshComponent, Projection};
use crate::render::lod::relative_height;
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

    /// Whether `mesh`, whose world bounds are `bounds`, is too far or too small for
    /// this view: past its `max_draw_distance` from the eye (to the bounds' centre),
    /// or below its `min_screen_size` ([`relative_height`] of the bounds' largest
    /// extent at that distance). `false` when it sets neither, or when measured from
    /// nowhere. "Too far" is tested first, so a mesh both culls counts once.
    pub(crate) fn culls(&self, mesh: &MeshComponent, (min, max): (Vec3, Vec3)) -> bool {
        let Some((eye, fov, projection)) = self.lens else {
            return false;
        };
        if !mesh.culls_when_small() {
            return false;
        }
        let distance = ((min + max) * 0.5).distance(eye);
        if mesh.max_draw_distance > 0.0 && distance > mesh.max_draw_distance {
            return true;
        }
        let size = (max - min).max_element();
        mesh.min_screen_size > 0.0
            && relative_height(size, distance, fov, projection) < mesh.min_screen_size
    }
}

#[cfg(test)]
#[path = "small_tests.rs"]
mod small_tests;
