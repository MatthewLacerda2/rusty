use super::Camera;
use crate::rules::lens as rules;
use rusty_components::{CameraComponent, Projection};

/// The largest near plane and orthographic half-height, in metres: below
/// `sqrt(f32::MAX)`, so `near * far` and the orthographic extents stay finite.
pub const MAX_LENS_EXTENT: f32 = 1e18;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub projection: Projection,
}

impl Lens {
    pub fn apply(self, cam: &mut Camera) {
        (cam.fov, cam.near, cam.far) = (self.fov, self.near, self.far);
        cam.projection = self.projection;
    }
}

pub fn held_lens(c: &CameraComponent) -> Lens {
    let (near, far) = rules::hold_clip_planes(c.near, c.far);
    let near = near.min(MAX_LENS_EXTENT);
    let projection = match rules::hold_projection(c.projection) {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: size.min(MAX_LENS_EXTENT),
        },
        p => p,
    };
    Lens {
        fov: rules::hold_fov(c.fov),
        near,
        // A finite projection; `f32::MAX / near` stays far past the capped near plane.
        far: far.min(f32::MAX / near),
        projection,
    }
}

pub fn held_fov(fov: f32) -> f32 {
    rules::hold_fov(fov)
}
