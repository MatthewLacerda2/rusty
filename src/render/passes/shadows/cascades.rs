//! Cascade fitting for the directional shadow (#435): the camera's depth range up to
//! the shadow distance is split into slices, and each slice gets its own orthographic
//! light volume, so the sun's shadow follows the camera at a resolution that is
//! finest up close.
//!
//! Each cascade bounds its slice with a **sphere**, not a box: a sphere's size does not
//! change as the camera turns, so the ortho extent is constant and texels never
//! resize (the first half of shimmer-free CSM). Its centre is then **snapped** in
//! light space to a grid an eighth of the map wide (the second half: texels only
//! ever move by whole-texel steps). The grid is coarse on purpose, and the volume carries
//! a margin that covers it — so a cascade's light matrix is unchanged until the
//! camera crosses a fair fraction of that cascade, which is what lets the static
//! caster bake be cached per cascade (#355).
//!
//! Pure CPU math on `glam` types, unit-tested below; the pass consumes it.

use glam::{Mat4, Vec3};

use crate::components::ShadowSettings;
use crate::scene::Camera;

/// The most cascades the shadow map holds (its array layers).
pub const MAX_CASCADES: usize = 4;
/// Blend between the practical split scheme's logarithmic (1.0) and uniform (0.0)
/// splits. Logarithmic spends resolution where perspective needs it; the uniform
/// share keeps the first slice from collapsing to a sliver near the lens.
const SPLIT_LAMBDA: f32 = 0.75;
/// Each cascade's band, as a fraction of its far split, over which the forward shader
/// cross-fades into the next cascade. The next cascade is fitted to start that far
/// back, so the band is covered by both.
pub const BLEND_FRACTION: f32 = 0.1;
/// Margin around the bounding sphere, as a fraction of its radius, that absorbs the
/// coarse centre snap. Costs this share of resolution, buys the static-bake cache.
const MARGIN: f32 = 0.25;
/// Snap grid, as a fraction of the map: `size / 8` texels is a whole-texel step (no
/// shimmer) for any size divisible by 8, and `step / 2 <= MARGIN * radius`, so the
/// snapped volume still contains the sphere.
const SNAP_FRACTION: f32 = 1.0 / 8.0;
/// How far toward the light, past a slice's sphere, casters are still captured: a
/// tall building or a cliff between the sun and the slice still shadows it. Sized to
/// a level (#435 cites ~150–250 m maps).
const PULLBACK: f32 = 250.0;

/// One cascade: the light's view-projection and what the forward shader needs to
/// pick it and bias against it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    pub light_space: Mat4,
    /// View depth (along the camera forward) where this cascade ends.
    pub split: f32,
    /// World size of one shadow-map texel.
    pub texel: f32,
    /// World depth the map's `[0, 1]` spans.
    pub depth_range: f32,
}

/// The practical split distances: the far edge of each of `count` slices of
/// `[near, far]`.
pub fn split_distances(near: f32, far: f32, count: usize) -> Vec<f32> {
    (1..=count)
        .map(|i| {
            let t = i as f32 / count as f32;
            let log = near * (far / near).powf(t);
            let uniform = near + (far - near) * t;
            SPLIT_LAMBDA * log + (1.0 - SPLIT_LAMBDA) * uniform
        })
        .collect()
}

/// Fit `settings.cascades` cascades of a `map_size`² map to `camera` (at `aspect`),
/// for a sun shining along `light_dir`.
pub fn fit(
    camera: &Camera,
    aspect: f32,
    light_dir: Vec3,
    settings: &ShadowSettings,
    map_size: u32,
) -> Vec<Cascade> {
    let near = camera.near.max(0.01);
    let far = settings.distance.min(camera.far).max(near * 2.0);
    let count = settings.cascades.clamp(1, MAX_CASCADES as u32) as usize;
    let light_view = light_view(light_dir);
    let mut start = near;
    split_distances(near, far, count)
        .into_iter()
        .map(|split| {
            let (centre, radius) = slice_sphere(camera, aspect, start, split);
            start = split * (1.0 - BLEND_FRACTION);
            snapped_volume(light_view, centre, radius, split, map_size)
        })
        .collect()
}

/// A world-origin view looking along `dir`, so snapping happens in a light-space
/// frame that does not move with the camera.
fn light_view(dir: Vec3) -> Mat4 {
    let dir = dir.normalize();
    let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    Mat4::look_at_rh(Vec3::ZERO, dir, up)
}

/// The bounding sphere of the camera frustum between view depths `near` and `far`.
/// Centre on the view axis where the nearest and farthest corners are equidistant
/// (clamped into the slice); radius rounded up to 1/16 so float noise in the camera
/// never resizes the cascade.
fn slice_sphere(camera: &Camera, aspect: f32, near: f32, far: f32) -> (Vec3, f32) {
    let tan_v = (camera.fov.to_radians() * 0.5).tan();
    // Squared half-diagonal of the slice cross-section per unit depth.
    let k = tan_v * tan_v * (1.0 + aspect * aspect);
    let along = (0.5 * (near + far) * (1.0 + k)).clamp(near, far);
    let corner = |d: f32| ((along - d).powi(2) + k * d * d).sqrt();
    let radius = (corner(near).max(corner(far)) * 16.0).ceil() / 16.0;
    (camera.position + camera.forward() * along, radius)
}

/// The ortho volume around a sphere, snapped in light space.
fn snapped_volume(light_view: Mat4, centre: Vec3, radius: f32, split: f32, size: u32) -> Cascade {
    let half = radius * (1.0 + MARGIN);
    let texel = 2.0 * half / size as f32;
    let step = 2.0 * half * SNAP_FRACTION;
    let snap = |v: f32| (v / step).round() * step;
    let c = light_view.transform_point3(centre);
    let (x, y, z) = (snap(c.x), snap(c.y), snap(c.z));
    // Light view looks down -Z: the volume spans view z from `z + half + PULLBACK`
    // (toward the light) to `z - half` (past the sphere), as ortho near/far distances.
    let (near, far) = (-z - half - PULLBACK, -z + half);
    let proj = Mat4::orthographic_rh(x - half, x + half, y - half, y + half, near, far);
    Cascade {
        light_space: proj * light_view,
        split,
        texel,
        depth_range: far - near,
    }
}

#[cfg(test)]
#[path = "cascades_tests.rs"]
mod tests;
