//! The lightmap path tracer (#438): what one texel's lightmap value is.
//!
//! A texel stores `E / π`: the irradiance reaching it, over π. The forward shader
//! multiplies it by albedo, so a lightmapped surface ends up `albedo · E / π`, the
//! same Lambert response a realtime light gives (`albedo / π · radiance · cos`). The
//! sky is a radiance of `sky` above the horizon fading to a quarter below, so an open
//! floor bakes to the flat ambient term an unlit one shows.
//!
//! Each sample shoots a cosine-weighted ray; the mean of the radiance the rays bring
//! back is exactly `E / π`. A ray that escapes returns the sky; one that hits a front
//! face returns that surface's emission plus `albedo / π` times its direct light (a
//! shadow ray per light), and keeps bouncing, `bounces` surfaces deep. A back face
//! returns nothing, so light never leaks through a closed wall.
//!
//! Alongside each part the tracer sums where the light came from (#810): every ray's
//! direction weighted by the luminance it brought back, so the bake can store the
//! dominant incoming direction that lets normal maps reshape the lightmap.

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use super::bvh::{Bvh, Hit};
use super::input::{BakeLight, BakeScene, LightShape};
use super::rng::Rng;

/// Ray offset along the surface normal, against self-intersection, in world units.
const SURFACE_OFFSET: f32 = 1e-3;
/// Nearest hit a ray accepts.
pub(super) const T_MIN: f32 = 1e-4;
/// The share of a texel's rays that may start out on back faces before the texel is
/// judged to be inside geometry (Unity's backface tolerance, inverted).
const BACKFACE_TOLERANCE: f32 = 0.5;

/// One texel's light, split so the bake can smooth the noisy part only.
#[derive(Clone, Copy, Debug)]
pub(super) struct TexelLight {
    pub indirect: Vec3,
    pub direct: Vec3,
    /// Luminance-weighted sum of the directions `indirect` and `direct` arrived
    /// from, in the same units: `|indirect_dir| <= luminance(indirect)` (#810).
    pub indirect_dir: Vec3,
    pub direct_dir: Vec3,
    /// False when most of the texel's rays start out hitting back faces: it sits
    /// inside other geometry (a floor texel under a crate), so its light is
    /// meaningless and the bake fills it from its neighbours instead (Unity's
    /// backface tolerance).
    pub valid: bool,
}

/// The static scene ready to trace.
pub(super) struct Tracer<'a> {
    pub scene: &'a BakeScene,
    pub bvh: &'a Bvh,
}

impl Tracer<'_> {
    /// The lightmap value at `position` facing `normal`, in its two parts: the bounce
    /// and sky over `samples` paths `bounces` deep, and the direct light of `Baked`
    /// lights. The bake filters the first and adds the second.
    pub(super) fn texel(
        &self,
        position: Vec3,
        normal: Vec3,
        samples: u32,
        bounces: u32,
        rng: &mut Rng,
    ) -> TexelLight {
        let origin = position + normal * SURFACE_OFFSET;
        let (mut sum, mut dir_sum, mut inside) = (Vec3::ZERO, Vec3::ZERO, 0);
        for _ in 0..samples {
            let (radiance, dir, backface) = self.path(origin, normal, bounces, rng);
            sum += radiance;
            dir_sum += dir * luminance(radiance);
            inside += u32::from(backface);
        }
        let n = samples.max(1) as f32;
        let (direct, direct_dir) = self.direct(origin, normal, true);
        TexelLight {
            indirect: sum / n,
            direct: direct / PI,
            indirect_dir: dir_sum / n,
            direct_dir: direct_dir / PI,
            valid: (inside as f32) < samples as f32 * BACKFACE_TOLERANCE,
        }
    }

    /// The radiance one cosine-weighted path from `origin` brings back, the
    /// direction its first ray left in, and whether that ray hit a back face.
    fn path(
        &self,
        mut origin: Vec3,
        mut normal: Vec3,
        bounces: u32,
        rng: &mut Rng,
    ) -> (Vec3, Vec3, bool) {
        let (mut radiance, mut throughput) = (Vec3::ZERO, Vec3::ONE);
        let mut first = Vec3::ZERO;
        for depth in 0..bounces.max(1) {
            let dir = cosine_sample(normal, rng);
            if depth == 0 {
                first = dir;
            }
            let Some(hit) = self.bvh.closest(origin, dir, T_MIN, f32::MAX) else {
                radiance += throughput * self.sky(dir);
                break;
            };
            let Some((point, hit_normal, mesh)) = self.surface(&hit, dir) else {
                return (radiance, first, depth == 0); // back face: nothing comes through
            };
            let m = &self.scene.meshes[mesh];
            let lit = self.direct(point, hit_normal, false).0;
            radiance += throughput * (m.emissive + m.albedo * lit / PI);
            throughput *= m.albedo;
            (origin, normal) = (point, hit_normal);
        }
        (radiance, first, false)
    }

    /// The offset hit point, its shading normal and mesh — `None` on a back face.
    fn surface(&self, hit: &Hit, dir: Vec3) -> Option<(Vec3, Vec3, usize)> {
        let tri = &self.bvh.tris[hit.tri];
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
        Some((point + n * SURFACE_OFFSET, n, tri.mesh as usize))
    }

    /// The direct irradiance at `point` facing `normal`, shadowed, and the
    /// luminance-weighted sum of the directions it arrived from. `baked_only` keeps
    /// just the `Baked` lights (a receiving texel's own direct light); a bounce hit
    /// sees every baked-in light.
    fn direct(&self, point: Vec3, normal: Vec3, baked_only: bool) -> (Vec3, Vec3) {
        let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
        for light in &self.scene.lights {
            if baked_only && !light.bakes_direct {
                continue;
            }
            if let Some((to_light, distance, received)) = arrival(light, point) {
                let cos = normal.dot(to_light);
                if cos > 0.0 && !self.bvh.occluded(point, to_light, T_MIN, distance) {
                    e += received * cos;
                    dir += to_light * luminance(received * cos);
                }
            }
        }
        (e, dir)
    }

    /// What a ray escaping along `dir` sees: the forward shader's ambient gradient.
    fn sky(&self, dir: Vec3) -> Vec3 {
        let ground = self.scene.sky * 0.25;
        ground.lerp(self.scene.sky, dir.y * 0.5 + 0.5)
    }
}

/// Rec. 709 luminance: how much a colour weighs when averaging light directions.
pub(super) fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// Unit direction to `light`, distance to it, and the radiance arriving at `point`
/// (before the cosine), with the forward shader's falloff: `1 / (d² + 1)` inside
/// `range`, and the spot's linear inner-to-outer cone fade. `None` out of reach.
pub(super) fn arrival(light: &BakeLight, point: Vec3) -> Option<(Vec3, f32, Vec3)> {
    match light.shape {
        LightShape::Directional { direction } => Some((-direction, f32::MAX, light.radiance)),
        LightShape::Point { position, range } => {
            let (dir, d) = toward(position, point, range)?;
            Some((dir, d, light.radiance / (d * d + 1.0)))
        }
        LightShape::Spot {
            position,
            direction,
            range,
            cos_inner,
            cos_outer,
        } => {
            let (dir, d) = toward(position, point, range)?;
            let theta = dir.dot(-direction);
            if theta <= cos_outer {
                return None;
            }
            let cone = ((theta - cos_outer) / (cos_inner - cos_outer).max(1e-4)).clamp(0.0, 1.0);
            Some((dir, d, light.radiance * cone / (d * d + 1.0)))
        }
    }
}

/// Unit direction and distance from `point` to `position`, if within `range`.
fn toward(position: Vec3, point: Vec3, range: f32) -> Option<(Vec3, f32)> {
    let offset = position - point;
    let d = offset.length();
    (d <= range && d > 0.0).then(|| (offset / d, d))
}

/// A cosine-weighted direction on the hemisphere around `normal`.
fn cosine_sample(normal: Vec3, rng: &mut Rng) -> Vec3 {
    let (u1, u2) = (rng.next_f32(), rng.next_f32());
    let r = u1.sqrt();
    let phi = TAU * u2;
    let local = Vec3::new(r * phi.cos(), r * phi.sin(), (1.0 - u1).max(0.0).sqrt());
    let (t, b) = normal.any_orthonormal_pair();
    (t * local.x + b * local.y + normal * local.z).normalize()
}
