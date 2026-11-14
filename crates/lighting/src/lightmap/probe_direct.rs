//! The direct light of `Baked` lights at light probes (#809), and a `Shadowmask`
//! bake's per-probe occlusion of its `Mixed` lights ([`mixed_occlusion`], #1695).
//!
//! A `Baked` light never renders live, so dynamic objects get it only through the
//! probes. The probe bake's cubemap capture sees lit *surfaces*, never a light itself,
//! so the light's own direct contribution is added here analytically: per probe, each
//! `Baked` light that reaches it unoccluded (one shadow ray against the static scene)
//! projects into the SH as a delta of the radiance arriving from it. `Sh9::eval` then
//! reads it back as `radiance · max(cos, 0)` (to L2 accuracy), the irradiance a
//! realtime light gives, so a dynamic object lit through the probe matches the Lambert
//! response the forward shader gives a live light. Under the `HalfLambert` diffuse
//! model (#1698) each band is rescaled (`DiffuseModel::sh_band_scale`) so the same
//! read-back gives the squared wrap instead, exactly: it is a quadratic in N·L. Same falloff and cones as the
//! lightmap tracer (`trace::arrival`). An Area light (#1674) is integrated over a
//! fixed grid of points on its rectangle, one shadow ray each (`area::grid`).

use glam::{Vec2, Vec3, Vec4};

use super::bake::triangles;
use super::input::{BakeScene, LightShape};
use super::lights::area;
use super::rays::Rays;
use super::trace::{arrival, Tracer, T_MIN};
use rusty_scene::lighting::sh::Sh9;

/// One SH per position in `positions`: the direct light of every `Baked` light in
/// `scene` (lights with `bakes_direct`), shadowed by its static meshes. Zero where no
/// `Baked` light reaches, and for every probe when the scene has none.
pub fn baked_direct_sh(scene: &BakeScene, positions: &[Vec3]) -> Vec<Sh9> {
    if !scene.lights.iter().any(|l| l.bakes_direct) {
        return vec![Sh9::zero(); positions.len()];
    }
    let rays = Rays::build(triangles(scene));
    positions
        .iter()
        .map(|&p| {
            let mut sh = Sh9::zero();
            for light in scene.lights.iter().filter(|l| l.bakes_direct) {
                let mut add = |u: Vec2, weight: f32| {
                    if let Some((to_light, distance, received)) = arrival(light, p, u) {
                        if !rays.occluded(p, to_light, T_MIN, distance) {
                            sh.add_sample(to_light, received, weight);
                        }
                    }
                };
                match light.shape {
                    // An Area light is integrated over a fixed grid of points (#1674).
                    LightShape::Area(area) if area.reaches(p) => {
                        let weight = 1.0 / (area::GRID * area::GRID) as f32;
                        area::grid().for_each(|u| add(u, weight));
                    }
                    LightShape::Area(_) => {}
                    _ => add(Vec2::ZERO, 1.0),
                }
            }
            sh.band_scaled(scene.diffuse.sh_band_scale())
        })
        .collect()
}

/// One shadowmask occlusion per position in `positions` (#1695, Unity's light-probe
/// occlusion): per channel, 0 where one shadow ray to its `Mixed` light meets a
/// static occluder, else 1, as the lightmap's mask texels are
/// (`trace::Tracer::mask`). All ones when the scene masks no light. A NaN position
/// reaches nothing, so it stays unoccluded.
pub fn mixed_occlusion(scene: &BakeScene, positions: &[Vec3]) -> Vec<Vec4> {
    if !scene.lights.iter().any(|l| l.mask_channel.is_some()) {
        return vec![Vec4::ONE; positions.len()];
    }
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    positions.iter().map(|&p| tracer.mask(p)).collect()
}
