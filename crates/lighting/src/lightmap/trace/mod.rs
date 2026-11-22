use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3, Vec4};

#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
mod check;
mod direct;
mod emitter;
#[cfg(test)]
mod emitter_tests;
mod grid;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod mis_tests;
mod own;
mod paths;
#[cfg(test)]
mod pick_tests;
mod sources;
mod validity;

pub(super) use direct::arrival;
pub(super) use paths::{Paths, Shown};

use super::input::{BakeRun, BakeScene};
use super::rays::{Hit, Rays};
use super::rng::Rng;
use direct::balance;
use rusty_core::color::luminance;
use sources::{Picks, Source, Sources};

const SURFACE_OFFSET: f32 = 1e-3;
pub(super) const T_MIN: f32 = 1e-4;
/// Cycles' *Clamp Indirect* default: a path is scaled down, hue kept, until its
/// brightest channel is at most this. The texel's own direct light is never clamped.
pub(super) const INDIRECT_CLAMP: f32 = 10.0;
/// Survival odds of Russian roulette: never below the low bound (no firefly
/// reweighting), never above the high one.
const ROULETTE: (f32, f32) = (0.05, 0.95);

#[derive(Clone, Copy, Debug)]
pub(super) struct TexelLight {
    pub indirect: Vec3,
    pub direct: Vec3,
    pub indirect_dir: Vec3,
    pub direct_dir: Vec3,
    pub mask: Vec4,
    pub valid: bool,
}

impl TexelLight {
    pub const INSIDE: Self = Self {
        indirect: Vec3::ZERO,
        direct: Vec3::ZERO,
        indirect_dir: Vec3::ZERO,
        direct_dir: Vec3::ZERO,
        mask: Vec4::ONE,
        valid: false,
    };
}

pub(super) struct Tracer<'a> {
    pub scene: &'a BakeScene,
    pub rays: &'a Rays,
    sources: Sources,
    probe: bool,
}

impl<'a> Tracer<'a> {
    pub(super) fn new(scene: &'a BakeScene, rays: &'a Rays) -> Self {
        let sources = Sources::build(scene, &rays.tris);
        Self {
            scene,
            rays,
            sources,
            probe: true,
        }
    }

    #[cfg(test)]
    pub(super) fn without_probe(self) -> Self {
        Self {
            probe: false,
            ..self
        }
    }

    fn path(
        &self,
        mut origin: Vec3,
        mut normal: Vec3,
        bounces: u32,
        rng: &mut Rng,
        picks: &mut Picks,
    ) -> (Vec3, Vec3) {
        let bounces = bounces.max(1);
        let (own, own_dir) = self.next_event(origin, normal, (false, true), rng, picks);
        let (mut radiance, mut throughput) = (Vec3::ZERO, Vec3::ONE);
        let mut first = Vec3::ZERO;
        for depth in 0..bounces {
            if depth >= 2 {
                let p = throughput.max_element().clamp(ROULETTE.0, ROULETTE.1);
                if rng.next_f32() >= p {
                    break;
                }
                throughput /= p;
            }
            let dir = cosine_sample(normal, rng);
            if depth == 0 {
                first = dir;
            }
            let Some(hit) = self.rays.closest(origin, dir, T_MIN, f32::MAX) else {
                radiance += throughput * self.sky(dir);
                break;
            };
            let Some((point, hit_normal, m)) = self.surface(&hit, dir) else {
                break;
            };
            radiance += throughput * self.glow(&hit, origin, normal, dir, picks);
            let more = depth + 1 < bounces;
            let lit = self
                .next_event(point, hit_normal, (true, more), rng, picks)
                .0;
            radiance += throughput * m.albedo * lit / PI;
            throughput *= m.albedo;
            (origin, normal) = (point, hit_normal);
        }
        let total = own / PI + radiance;
        let clamped = clamp_indirect(total);
        let scale = luminance(clamped) / luminance(total).max(f32::MIN_POSITIVE);
        let dir = (own_dir / PI + first * luminance(radiance)) * scale.min(1.0);
        (clamped, dir)
    }

    fn glow(&self, hit: &Hit, origin: Vec3, normal: Vec3, dir: Vec3, picks: &Picks) -> Vec3 {
        let tri = &self.rays.tris[hit.tri];
        let Some(k) = self.sources.emitter_at(self.scene, tri) else {
            return Vec3::ZERO;
        };
        let emitter = &self.sources.emitters[k];
        let at = tri.a + tri.e1 * hit.u + tri.e2 * hit.v;
        let cos_emit = -dir.dot(tri.normal().normalize_or_zero());
        let light = picks.chance(Source::Emitter(k)) * emitter.pdf(at.distance(origin), cos_emit);
        emitter.radiance() * balance(normal.dot(dir) / PI, light)
    }

    fn surface(&self, hit: &Hit, dir: Vec3) -> Option<(Vec3, Vec3, &BakeRun)> {
        let tri = &self.rays.tris[hit.tri];
        if tri.normal().dot(dir) >= 0.0 {
            return None;
        }
        let mesh = &self.scene.meshes[tri.mesh as usize];
        let at = |k: u32| mesh.indices[(tri.first + k) as usize] as usize;
        let w = Vec3::new(1.0 - hit.u - hit.v, hit.u, hit.v);
        let n = (w.x * mesh.normals[at(0)] + w.y * mesh.normals[at(1)] + w.z * mesh.normals[at(2)])
            .try_normalize()
            .unwrap_or(tri.normal().try_normalize().unwrap_or(Vec3::Y));
        let point = tri.a + tri.e1 * hit.u + tri.e2 * hit.v;
        Some((point + n * SURFACE_OFFSET, n, mesh.run_at(tri.first)))
    }

    pub(super) fn mask(&self, point: Vec3) -> Vec4 {
        let mut mask = Vec4::ONE;
        for light in &self.scene.lights {
            let Some(c) = light.mask_channel else {
                continue;
            };
            if let Some((to_light, distance, _)) = arrival(light, point, Vec2::ZERO) {
                if self.rays.occluded(point, to_light, T_MIN, distance) {
                    mask[c as usize] = 0.0;
                }
            }
        }
        mask
    }

    fn sky(&self, dir: Vec3) -> Vec3 {
        // A trilight's `incoming` rings below zero toward the poles; unclamped it would
        // break `|indirect_dir| <= luminance(indirect)`.
        self.scene.sky.incoming(dir).max(Vec3::ZERO)
    }
}

pub(super) fn clamp_indirect(radiance: Vec3) -> Vec3 {
    let peak = radiance.max_element();
    if peak > INDIRECT_CLAMP {
        radiance * (INDIRECT_CLAMP / peak)
    } else {
        radiance
    }
}

pub(crate) fn cosine_sample(normal: Vec3, rng: &mut Rng) -> Vec3 {
    let (u1, u2) = (rng.next_f32(), rng.next_f32());
    let r = u1.sqrt();
    let phi = TAU * u2;
    let local = Vec3::new(r * phi.cos(), r * phi.sin(), (1.0 - u1).max(0.0).sqrt());
    let (t, b) = normal.any_orthonormal_pair();
    (t * local.x + b * local.y + normal * local.z).normalize()
}
