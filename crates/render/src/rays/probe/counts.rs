//! What a camera's rays do in the tree, counted on the shader's CPU twin.

use glam::{Mat4, Vec3, Vec4};

use crate::rays::walk::{counted, Counts};
use crate::rays::Packed;

/// One pixel in `STRIDE` along each axis is counted.
const STRIDE: u32 = 8;

/// The mean a ray, of a camera's rays as `rt/probe.wgsl`'s `pixel_ray` makes them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerRay {
    pub instances: f64,
    pub nodes: f64,
    pub tris: f64,
}

pub(super) fn per_ray(p: &Packed, inv_view_proj: Mat4, eye: Vec3, size: (u32, u32)) -> PerRay {
    let mut sum = Counts::default();
    let mut rays = 0u64;
    for y in (0..size.1).step_by(STRIDE as usize) {
        for x in (0..size.0).step_by(STRIDE as usize) {
            let uv =
                (glam::vec2(x as f32, y as f32) + 0.5) / glam::vec2(size.0 as f32, size.1 as f32);
            let at = inv_view_proj * Vec4::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.5, 1.0);
            let dir = (at.truncate() / at.w - eye).normalize();
            counted(p, (eye, dir, 1e6), &mut sum);
            rays += 1;
        }
    }
    let mean = |n: u64| n as f64 / rays.max(1) as f64;
    PerRay {
        instances: mean(sum.instances),
        nodes: mean(sum.nodes),
        tris: mean(sum.tris),
    }
}
