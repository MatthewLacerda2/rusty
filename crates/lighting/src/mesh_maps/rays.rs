use glam::{Vec3, Vec4};

use super::{Baked, MeshSource};
use crate::lightmap::{cosine_sample, Rays, Rng, TexelPoint, Tri};

/// Rays per texel: Substance's default *Secondary Rays*.
pub const SAMPLES: u32 = 64;

/// A share of the bounds diagonal: Substance's *Max Occluder Distance*.
pub const MAX_DISTANCE: f32 = 1.0;

/// A share of the bounds diagonal, so a ray misses its own triangle at any scale.
const OFFSET: f32 = 1e-4;

/// One seed stream per map, so AO and thickness draw apart.
const AO_STREAM: u64 = 1;
const THICKNESS_STREAM: u64 = 2;

pub(super) struct MeshRays {
    rays: Rays,
}

impl MeshRays {
    pub(super) fn new(source: &MeshSource) -> Self {
        let p = &source.positions;
        let tris = source
            .indices
            .chunks_exact(3)
            .enumerate()
            .map(|(k, t)| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| p[i as usize]);
                Tri::new(a, b, c, 0, 3 * k as u32)
            })
            .collect();
        Self {
            rays: Rays::build(tris),
        }
    }

    pub(super) fn ao(&self, baked: &Baked, i: usize, p: &TexelPoint) -> Vec4 {
        let reach = MAX_DISTANCE * baked.diagonal;
        let mut rng = Rng::new(0, AO_STREAM, i as u64);
        let (origin, dirs) = rays_off(baked, p, 1.0, baked.settings.ao_rays, &mut rng);
        let (mut open, mut cast) = (0u32, 0u32);
        for dir in dirs {
            cast += 1;
            open += u32::from(!self.rays.occluded_by(origin, dir, (0.0, reach), |_| true));
        }
        let open = if cast == 0 {
            1.0
        } else {
            open as f32 / cast as f32
        };
        Vec3::splat(open).extend(1.0)
    }

    pub(super) fn thickness(&self, baked: &Baked, i: usize, p: &TexelPoint) -> Vec4 {
        let reach = MAX_DISTANCE * baked.diagonal;
        let mut rng = Rng::new(0, THICKNESS_STREAM, i as u64);
        let n = baked.settings.thickness_rays;
        let (origin, dirs) = rays_off(baked, p, -1.0, n, &mut rng);
        let (mut travelled, mut cast) = (0.0f32, 0u32);
        for dir in dirs {
            cast += 1;
            travelled += self.distance(origin, dir, reach);
        }
        let thick = if cast == 0 {
            1.0
        } else {
            travelled / (cast as f32 * reach)
        };
        Vec3::splat(thick).extend(1.0)
    }

    fn distance(&self, origin: Vec3, dir: Vec3, reach: f32) -> f32 {
        let Some(hit) = self.rays.closest(origin, dir, 0.0, reach) else {
            return reach;
        };
        let t = &self.rays.tris[hit.tri];
        let point = t.a + t.e1 * hit.u + t.e2 * hit.v;
        point.distance(origin).min(reach)
    }
}

fn rays_off<'r>(
    baked: &Baked,
    p: &TexelPoint,
    side: f32,
    samples: u32,
    rng: &'r mut Rng,
) -> (Vec3, impl Iterator<Item = Vec3> + 'r) {
    let face = baked.source.face_normal(p.tri()).unwrap_or(p.normal);
    let around = if p.normal.dot(face) > 0.0 {
        p.normal
    } else {
        face
    } * side;
    let face = face * side;
    let origin = p.position + face * OFFSET * baked.diagonal;
    let dirs = (0..samples)
        .map(move |_| cosine_sample(around, rng))
        .filter(move |d| d.dot(face) > 0.0);
    (origin, dirs)
}
