//! A `Baked` light never renders live and the probe capture sees surfaces, never a light, so each
//! one reaching a probe unoccluded is added as an SH delta that `Sh9::eval` reads back as `radiance
//! · max(cos, 0)` to L2 accuracy. `HalfLambert` rescales each band (`DiffuseModel::sh_band_scale`)
//! so the read-back is the squared wrap exactly. Falloff and cones match `trace::arrival`.

use glam::{Vec2, Vec3, Vec4};

use super::bake::triangles;
use super::input::{BakeScene, LightShape};
use super::lights::area;
use super::rays::Rays;
use super::trace::{arrival, Tracer, T_MIN};
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::Scene;

/// On the channels the last bake recorded, never assigned here.
pub fn probe_direct(scene: &Scene, positions: &[Vec3]) -> (Vec<Sh9>, Vec<Vec4>) {
    let mut statics = BakeScene::gather(scene, &|_, _| None);
    statics.use_recorded_channels(scene);
    (
        baked_direct_sh(&statics, positions),
        mixed_occlusion(&statics, positions),
    )
}

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

/// Per channel, 0 where one shadow ray to its `Mixed` light meets a static occluder, else 1. A NaN
/// position stays unoccluded.
pub fn mixed_occlusion(scene: &BakeScene, positions: &[Vec3]) -> Vec<Vec4> {
    if !scene.lights.iter().any(|l| l.mask_channel.is_some()) {
        return vec![Vec4::ONE; positions.len()];
    }
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    positions.iter().map(|&p| tracer.mask(p)).collect()
}
