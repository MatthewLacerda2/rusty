//! Each slice is bounded by a sphere, constant as the camera turns, snapped coarsely in light
//! space so a cascade's static bake can be cached.

use glam::camera::rh::{proj::directx, view::look_at_mat4};
use glam::{Mat4, Vec3};

use rusty_components::ShadowSettings;
use rusty_scene::Camera;

pub const MAX_CASCADES: usize = ShadowSettings::MAX_CASCADES as usize;
/// Log (1) to uniform (0) split blend: the uniform share keeps the first slice off the lens.
const SPLIT_LAMBDA: f32 = 0.75;
/// Of a cascade's far split: the cross-fade band, covered by the next cascade too.
pub const BLEND_FRACTION: f32 = 0.1;
/// Of the sphere's radius.
const MARGIN: f32 = 0.25;
/// Of the map: whole-texel steps for a size divisible by 8, and half a step within `MARGIN`.
const SNAP_FRACTION: f32 = 1.0 / 8.0;
/// Metres toward the light past a slice where casters still shadow it.
const PULLBACK: f32 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    pub light_space: Mat4,
    pub split: f32,
    pub texel: f32,
    pub depth_range: f32,
}

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

pub fn fit(
    camera: &Camera,
    aspect: f32,
    light_dir: Vec3,
    settings: &ShadowSettings,
    map_size: u32,
) -> Vec<Cascade> {
    let near = camera.near;
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

fn light_view(dir: Vec3) -> Mat4 {
    let dir = dir.normalize();
    let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    look_at_mat4(Vec3::ZERO, dir, up)
}

fn slice_sphere(camera: &Camera, aspect: f32, near: f32, far: f32) -> (Vec3, f32) {
    let tan_v = (camera.fov.to_radians() * 0.5).tan();
    let k = tan_v * tan_v * (1.0 + aspect * aspect);
    let along = (0.5 * (near + far) * (1.0 + k)).clamp(near, far);
    let corner = |d: f32| ((along - d).powi(2) + k * d * d).sqrt();
    // Up to 1/16 m, so float noise never resizes a cascade.
    let radius = (corner(near).max(corner(far)) * 16.0).ceil() / 16.0;
    (camera.position + camera.forward() * along, radius)
}

fn snapped_volume(light_view: Mat4, centre: Vec3, radius: f32, split: f32, size: u32) -> Cascade {
    let half = radius * (1.0 + MARGIN);
    let texel = 2.0 * half / size as f32;
    let step = 2.0 * half * SNAP_FRACTION;
    let snap = |v: f32| (v / step).round() * step;
    let c = light_view.transform_point3(centre);
    let (x, y, z) = (snap(c.x), snap(c.y), snap(c.z));
    let (near, far) = (-z - half - PULLBACK, -z + half);
    let proj = directx::orthographic(x - half, x + half, y - half, y + half, near, far);
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
