use super::sanitize::{at_least, saturated};
use rusty_components::Projection;

pub const MIN_FOV: f32 = 1.0;
pub const MAX_FOV: f32 = 179.0;
/// The nearest a near plane may sit (Unity's inspector floor): a viewmodel's
/// 0.01 is allowed, 0 (an infinite depth range) is not.
pub const MIN_NEAR: f32 = 0.01;
pub const MIN_CLIP_GAP: f32 = 0.01;
pub const MIN_ORTHO_SIZE: f32 = 0.01;

/// A vertical field of view (degrees) clamped to [`MIN_FOV`]..=[`MAX_FOV`]. A NaN
/// takes [`MIN_FOV`] (`f32::clamp` would keep it).
pub fn hold_fov(fov: f32) -> f32 {
    saturated(fov, MIN_FOV, MAX_FOV)
}

pub fn hold_clip_planes(near: f32, far: f32) -> (f32, f32) {
    let near = at_least(near, MIN_NEAR).min(f32::MAX.next_down());
    let min_far = (near + MIN_CLIP_GAP).max(near.next_up());
    (near, at_least(far, min_far))
}

// lint: allow(sanitize) holds `projection`: its one float, the orthographic size, goes through `at_least`
pub fn hold_projection(projection: Projection) -> Projection {
    match projection {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: at_least(size, MIN_ORTHO_SIZE),
        },
        p => p,
    }
}
