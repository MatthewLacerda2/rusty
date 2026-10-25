//! The view camera's lens, held once where it is copied from a `CameraComponent`
//! (#1359). The authoring ops hold every *write*; a lens that skipped them (a scene
//! file, a prefab) is held here, through those same ops, so there is one set of
//! bounds and the view [`Camera`] is always valid. The stored component keeps
//! what it carries.

use super::Camera;
use crate::components::{CameraComponent, Projection};
use crate::scene::authoring::rendering::camera as ops;

/// The largest near plane and orthographic half-height the view camera takes:
/// below `sqrt(f32::MAX)`, so `near * far` (the perspective build) and the
/// orthographic extents stay finite. A plain number far beyond any level.
pub const MAX_LENS_EXTENT: f32 = 1e18;

/// A component's lens as the view camera uses it. See [`held_lens`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub projection: Projection,
}

impl Lens {
    /// Copy the lens onto `cam`, keeping its pose, mask and clear flags.
    pub fn apply(self, cam: &mut Camera) {
        (cam.fov, cam.near, cam.far) = (self.fov, self.near, self.far);
        cam.projection = self.projection;
    }
}

/// `c`'s lens held by the authoring ops (as if it had been written through
/// `set_fov`, `set_clip_planes` and `set_projection`), then capped at
/// [`MAX_LENS_EXTENT`] with `far` at most `f32::MAX / near`, so the projection
/// built from it is finite. A lens the ops wrote comes back unchanged.
pub fn held_lens(c: &CameraComponent) -> Lens {
    // A scratch component, so the bounds are the ops' own, not a copy of them.
    let mut h = CameraComponent::default();
    ops::set_fov(&mut h, c.fov);
    ops::set_clip_planes(&mut h, c.near, c.far);
    ops::set_projection(&mut h, c.projection);
    // `near` is finite and at least MIN_NEAR here; after the cap `f32::MAX / near`
    // is still far beyond it, so the far plane stays past the near one.
    let near = h.near.min(MAX_LENS_EXTENT);
    let projection = match h.projection {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: size.min(MAX_LENS_EXTENT),
        },
        p => p,
    };
    Lens {
        fov: h.fov,
        near,
        far: h.far.min(f32::MAX / near),
        projection,
    }
}

/// `fov` held the way [`held_lens`] holds a component's (the view camera's
/// `Camera.SetFov`).
pub fn held_fov(fov: f32) -> f32 {
    let mut h = CameraComponent::default();
    ops::set_fov(&mut h, fov);
    h.fov
}
