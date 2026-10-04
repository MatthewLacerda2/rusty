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

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use super::bvh::{Bvh, Hit};
use super::input::{BakeLight, BakeScene, LightShape};
use super::rng::Rng;

/// Ray offset along the surface normal, against self-intersection, in world units.
const SURFACE_OFFSET: f32 = 1e-3;
/// Nearest hit a ray accepts.
const T_MIN: f32 = 1e-4;

/// The static scene ready to trace.
pub(super) struct Tracer<'a> {
    pub scene: &'a BakeScene,
    pub bvh: &'a Bvh,
}

impl Tracer<'_> {
    /// The lightmap value at `position` facing `normal`: the bounce and sky over
    /// `samples` paths `bounces` deep, plus the direct light of `Baked` lights.
    pub(super) fn texel(
        &self,
        position: Vec3,
        normal: Vec3,
        samples: u32,
        bounces: u32,
        rng: &mut Rng,
    ) -> Vec3 {
        let origin = position + normal * SURFACE_OFFSET;
        let mut sum = Vec3::ZERO;
        for _ in 0..samples {
            sum += self.path(origin, normal, bounces, rng);
        }
        let indirect = sum / samples.max(1) as f32;
        indirect + self.direct(origin, normal, true) / PI
    }

    /// The radiance one cosine-weighted path from `origin` brings back.
    fn path(&self, mut origin: Vec3, mut normal: Vec3, bounces: u32, rng: &mut Rng) -> Vec3 {
        let (mut radiance, mut throughput) = (Vec3::ZERO, Vec3::ONE);
        for _ in 0..bounces.max(1) {
            let dir = cosine_sample(normal, rng);
            let Some(hit) = self.bvh.closest(origin, dir, T_MIN, f32::MAX) else {
                radiance += throughput * self.sky(dir);
                break;
            };
            let Some((point, hit_normal, mesh)) = self.surface(&hit, dir) else {
                break; // back face: nothing comes through
            };
            let m = &self.scene.meshes[mesh];
            radiance +=
                throughput * (m.emissive + m.albedo * self.direct(point, hit_normal, false) / PI);
            throughput *= m.albedo;
            (origin, normal) = (point, hit_normal);
        }
        radiance
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

    /// The direct irradiance at `point` facing `normal`, shadowed. `baked_only` keeps
    /// just the `Baked` lights (a receiving texel's own direct light); a bounce hit
    /// sees every baked-in light.
    fn direct(&self, point: Vec3, normal: Vec3, baked_only: bool) -> Vec3 {
        let mut e = Vec3::ZERO;
        for light in &self.scene.lights {
            if baked_only && !light.bakes_direct {
                continue;
            }
            if let Some((to_light, distance, received)) = arrival(light, point) {
                let cos = normal.dot(to_light);
                if cos > 0.0 && !self.bvh.occluded(point, to_light, T_MIN, distance) {
                    e += received * cos;
                }
            }
        }
        e
    }

    /// What a ray escaping along `dir` sees: the forward shader's ambient gradient.
    fn sky(&self, dir: Vec3) -> Vec3 {
        let ground = self.scene.sky * 0.25;
        ground.lerp(self.scene.sky, dir.y * 0.5 + 0.5)
    }
}

/// Unit direction to `light`, distance to it, and the radiance arriving at `point`
/// (before the cosine), with the forward shader's falloff: `1 / (d² + 1)` inside
/// `range`, and the spot's linear inner-to-outer cone fade. `None` out of reach.
fn arrival(light: &BakeLight, point: Vec3) -> Option<(Vec3, f32, Vec3)> {
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
