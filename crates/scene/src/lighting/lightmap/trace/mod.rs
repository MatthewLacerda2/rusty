//! The lightmap path tracer (#438): what one texel's lightmap value is.
//!
//! A texel stores `E / π`: the irradiance reaching it, over π. The forward shader
//! multiplies it by albedo, so a lightmapped surface ends up `albedo · E / π`, the
//! same response a realtime light gives (`albedo / π · radiance · cos`, the cosine
//! reshaped by the scene's `DiffuseModel`, #1698, in both). The
//! sky is the ambient's radiance by direction (`AmbientRadiance::incoming`), built so
//! its cosine-weighted average is the shader's flat ambient: an open surface bakes to
//! the ambient an unbaked one shows, whichever way it faces (#1647).
//!
//! Each sample shoots a cosine-weighted ray; the mean of the radiance the rays bring
//! back is exactly `E / π`. A ray that escapes returns the sky; one that hits a front
//! face returns `albedo / π` times its direct light and keeps bouncing, `bounces`
//! surfaces deep. That direct light is one next-event sample (#1883, `direct`): one
//! lamp or emissive surface picked by how much it could light the point, its light
//! over the pick's probability, so the cost follows the sources in range, not the
//! level's (`grid`), and a small bright panel is aimed at rather than found by luck
//! (`emitter`). An emitter a ray lands on adds its glow, weighed by MIS against the
//! sample that could have picked it, so no light is counted twice. Past the second
//! bounce, Russian roulette ends dim paths early. A back face
//! returns nothing, so light never leaks through a closed wall. A path's light is
//! clamped at [`INDIRECT_CLAMP`] (Cycles' *Clamp Indirect*, #1699), so a lamp hung
//! against a surface bakes no speckle. The texel's own direct light (its `Baked`
//! lamps) is not clamped, and not sampled either: never filtered, it counts every
//! lamp in range.
//!
//! Alongside each part the tracer sums where the light came from (#810): every ray's
//! direction weighted by the luminance it brought back, so the bake can store the
//! dominant incoming direction that lets normal maps reshape the lightmap.

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3, Vec4};

// A test hook: `dev::bake`'s per-texel cost measurement.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
mod direct;
mod emitter;
#[cfg(test)]
mod emitter_tests;
mod grid;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod mis_tests;
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

/// Ray offset along the surface normal, against self-intersection, in world units.
const SURFACE_OFFSET: f32 = 1e-3;
/// Nearest hit a ray accepts.
pub(super) const T_MIN: f32 = 1e-4;
/// The brightest one bounce path may come back (#1699), Blender Cycles' *Clamp
/// Indirect* at its default of 10: a path is scaled down, hue kept, until its
/// brightest channel is at most this. Inverse-square light has no ceiling, so a
/// lamp hung 5 cm under a ceiling lights the spot beside it hundreds of times
/// brighter than the rest; a path that happens to hit there would add a speckle
/// the bake's filter only smears. The texel's own direct light is never clamped.
pub(super) const INDIRECT_CLAMP: f32 = 10.0;
/// The bounds a path's survival odds are held to under Russian roulette (#1883):
/// never below the low one, so a dim path is never reweighted into a firefly, and
/// never above the high one, so even a bright path's depth costs a little less.
const ROULETTE: (f32, f32) = (0.05, 0.95);

/// One texel's light, split so the bake can smooth the noisy part only.
#[derive(Clone, Copy, Debug)]
pub(super) struct TexelLight {
    pub indirect: Vec3,
    pub direct: Vec3,
    /// Luminance-weighted sum of the directions `indirect` and `direct` arrived
    /// from, in the same units: `|indirect_dir| <= luminance(indirect)` (#810).
    pub indirect_dir: Vec3,
    pub direct_dir: Vec3,
    /// Per shadowmask channel (#1695), how much of its `Mixed` light reaches the
    /// texel past static occluders: 1 lit (or out of its reach), 0 shadowed.
    pub mask: Vec4,
    /// False when the texel sits inside other geometry (a floor texel under a
    /// crate): its light is meaningless, and the bake fills it from its own chart
    /// instead (#1949). [`Tracer::texel`] leaves it true; the bake asks
    /// [`Tracer::valid`] first, a test of its own that the sample count never
    /// moves, and gives an invalid texel [`TexelLight::INSIDE`] untraced.
    pub valid: bool,
}

impl TexelLight {
    /// A texel inside geometry, its paths never traced: the bake drops it and fills
    /// it from its own chart (#1949).
    pub const INSIDE: Self = Self {
        indirect: Vec3::ZERO,
        direct: Vec3::ZERO,
        indirect_dir: Vec3::ZERO,
        direct_dir: Vec3::ZERO,
        mask: Vec4::ONE,
        valid: false,
    };
}

/// The static scene ready to trace.
pub(super) struct Tracer<'a> {
    pub scene: &'a BakeScene,
    pub rays: &'a Rays,
    /// The lamps by reach and the emitters, what next-event estimation picks from.
    sources: Sources,
}

impl<'a> Tracer<'a> {
    /// The tracer over `scene`, its rays cast against `rays`.
    pub(super) fn new(scene: &'a BakeScene, rays: &'a Rays) -> Self {
        let sources = Sources::build(scene, &rays.tris);
        Self {
            scene,
            rays,
            sources,
        }
    }

    /// The radiance one path from `origin` brings back and its luminance-weighted
    /// direction (#810). The texel's
    /// own view of the emitters is one next-event sample; then a cosine-weighted
    /// ray, and at each front face it lands on, `albedo / π` times one next-event
    /// sample of the lamps (and the emitters, where another bounce follows),
    /// `bounces` surfaces deep (#1883). A ray that lands on an emitter adds its
    /// glow, weighed against the next-event sample that could have picked that
    /// point (MIS, the balance heuristic), so emitted light is counted once and
    /// stays smooth whether a panel is small or a whole floor. Emission reaches
    /// `bounces` surfaces deep and lamps one deeper, as they always did.
    /// After the second bounce a path goes on with probability `p`, its throughput
    /// held to [`ROULETTE`], and a survivor's throughput is divided by `p`.
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
                // A back face: nothing comes through.
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

    /// The glow a bounce ray from `origin` (facing `normal`) along `dir` finds at
    /// `hit`, times its MIS share against the next-event sample `picks` drew there:
    /// zero off an emitter.
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

    /// The offset hit point, its shading normal and the material run it landed on
    /// (#1290) — `None` on a back face.
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

    /// The shadowmask at `point` (#1695): per channel, 0 where one shadow ray to
    /// its light meets a static occluder that blocks light, else 1. A texel out of
    /// the light's reach keeps 1, so a channel two far-apart lights share
    /// (`lights::channels`) holds each one's shadow where it shines. Unity bakes
    /// occlusion the same way: visibility, not light, so it multiplies a live
    /// light's attenuation.
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

    /// What a ray escaping along `dir` sees: the ambient's radiance from there,
    /// clamped at zero per ray. A trilight's `incoming` can ring below zero toward
    /// the poles; summed unclamped, an escaping ray would subtract bounce light and
    /// its direction, breaking `|indirect_dir| <= luminance(indirect)` (#810).
    fn sky(&self, dir: Vec3) -> Vec3 {
        self.scene.sky.incoming(dir).max(Vec3::ZERO)
    }
}

/// `radiance` scaled down, hue kept, so no channel passes [`INDIRECT_CLAMP`].
pub(super) fn clamp_indirect(radiance: Vec3) -> Vec3 {
    let peak = radiance.max_element();
    if peak > INDIRECT_CLAMP {
        radiance * (INDIRECT_CLAMP / peak)
    } else {
        radiance
    }
}

/// A cosine-weighted direction on the hemisphere around `normal`.
pub(in crate::lighting) fn cosine_sample(normal: Vec3, rng: &mut Rng) -> Vec3 {
    let (u1, u2) = (rng.next_f32(), rng.next_f32());
    let r = u1.sqrt();
    let phi = TAU * u2;
    let local = Vec3::new(r * phi.cos(), r * phi.sin(), (1.0 - u1).max(0.0).sqrt());
    let (t, b) = normal.any_orthonormal_pair();
    (t * local.x + b * local.y + normal * local.z).normalize()
}
