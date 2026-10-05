//! The direct light of `Baked` lights at light probes (#809).
//!
//! A `Baked` light never renders live, so dynamic objects get it only through the
//! probes. The probe bake's cubemap capture sees lit *surfaces*, never a light itself,
//! so the light's own direct contribution is added here analytically: per probe, each
//! `Baked` light that reaches it unoccluded (one shadow ray against the static scene)
//! projects into the SH as a delta of the radiance arriving from it. `Sh9::eval` then
//! reads it back as `radiance · max(cos, 0)` (to L2 accuracy), the irradiance a
//! realtime light gives, so a dynamic object lit through the probe matches the Lambert
//! response the forward shader gives a live light. Same falloff and cones as the
//! lightmap tracer (`trace::arrival`).

use glam::Vec3;

use super::bake::triangles;
use super::bvh::Bvh;
use super::input::BakeScene;
use super::trace::{arrival, T_MIN};
use crate::scene::lighting::sh::Sh9;

/// One SH per position in `positions`: the direct light of every `Baked` light in
/// `scene` (lights with `bakes_direct`), shadowed by its static meshes. Zero where no
/// `Baked` light reaches, and for every probe when the scene has none.
pub fn baked_direct_sh(scene: &BakeScene, positions: &[Vec3]) -> Vec<Sh9> {
    if !scene.lights.iter().any(|l| l.bakes_direct) {
        return vec![Sh9::zero(); positions.len()];
    }
    let bvh = Bvh::build(triangles(scene));
    positions
        .iter()
        .map(|&p| {
            let mut sh = Sh9::zero();
            for light in scene.lights.iter().filter(|l| l.bakes_direct) {
                if let Some((to_light, distance, received)) = arrival(light, p) {
                    if !bvh.occluded(p, to_light, T_MIN, distance) {
                        sh.add_sample(to_light, received, 1.0);
                    }
                }
            }
            sh
        })
        .collect()
}
