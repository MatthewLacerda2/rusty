//! crates/scene/src/rules/lens.rs — a camera lens's bounds (#1359): Unity's Camera
//! inspector bounds (FOV 1–179°, near above 0, far beyond near), held on every write.
//!
//! The camera ops (`authoring::rendering::camera`) store through these, and the
//! view camera holds a lens that skipped them (a scene file, a prefab) through the
//! same functions (`camera::lens::held_lens`), so there is one set of bounds.
//!
//! Pure.

use super::sanitize::{at_least, saturated};
use rusty_components::Projection;

/// The field-of-view range, in degrees: Unity's Camera inspector bounds, applied
/// to every write (the view camera's `Camera.SetFov` too).
pub const MIN_FOV: f32 = 1.0;
pub const MAX_FOV: f32 = 179.0;
/// The nearest a near plane may sit (Unity's inspector floor): a viewmodel's
/// 0.01 is allowed, 0 (an infinite depth range) is not.
pub const MIN_NEAR: f32 = 0.01;
/// How far beyond the near plane the far plane must at least sit (Unity's gap).
pub const MIN_CLIP_GAP: f32 = 0.01;
/// The smallest orthographic half-height, so the view keeps a volume.
pub const MIN_ORTHO_SIZE: f32 = 0.01;

/// A vertical field of view (degrees) clamped to [`MIN_FOV`]..=[`MAX_FOV`]. A NaN
/// takes [`MIN_FOV`] (`f32::clamp` would keep it).
pub fn hold_fov(fov: f32) -> f32 {
    saturated(fov, MIN_FOV, MAX_FOV)
}

/// Both clip planes the way Unity holds them: `near` at least [`MIN_NEAR`], `far`
/// at least [`MIN_CLIP_GAP`] beyond it (a near plane past the far one pushes the
/// far one out). A non-finite plane takes its lower bound, and the far plane
/// always sits strictly beyond the near one, even where `near + MIN_CLIP_GAP`
/// rounds back to `near`.
pub fn hold_clip_planes(near: f32, far: f32) -> (f32, f32) {
    // Below f32::MAX, so the plane just beyond it is still finite.
    let near = at_least(near, MIN_NEAR).min(f32::MAX.next_down());
    let min_far = (near + MIN_CLIP_GAP).max(near.next_up());
    (near, at_least(far, min_far))
}

/// A projection with an orthographic size of at least [`MIN_ORTHO_SIZE`].
// lint: allow(sanitize) holds `projection`: its one float, the orthographic size, goes through `at_least`
pub fn hold_projection(projection: Projection) -> Projection {
    match projection {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: at_least(size, MIN_ORTHO_SIZE),
        },
        p => p,
    }
}
