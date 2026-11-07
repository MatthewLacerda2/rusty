//! The two ray-traced mesh maps (#1849), ambient occlusion and thickness, cast
//! through the lightmapper's `Rays` against the model alone.

use glam::{Vec3, Vec4};

use super::{Baked, MeshSource};
use crate::scene::lighting::lightmap::{cosine_sample, Rays, Rng, TexelPoint, Tri};

/// Rays per texel: Substance's default *Secondary Rays*.
pub const SAMPLES: u32 = 64;

/// How far a ray looks for an occluder, as a share of the mesh's bounds diagonal:
/// Substance's *Max Occluder Distance* relative to the bounding box.
pub const MAX_DISTANCE: f32 = 1.0;

/// The ray offset off the surface, as a share of the bounds diagonal, so a ray never
/// hits the triangle it starts on whatever the model's scale.
const OFFSET: f32 = 1e-4;

/// A seed per map, so AO and thickness draw from different streams.
const AO_STREAM: u64 = 1;
const THICKNESS_STREAM: u64 = 2;

/// The model's triangles, ready to trace.
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

    /// Ambient occlusion of the texel at `p` (`i` its place in the bake): the share
    /// of [`SAMPLES`] cosine rays off the surface that meet no triangle within
    /// [`MAX_DISTANCE`]. 1 is open, 0 enclosed.
    pub(super) fn ao(&self, baked: &Baked, i: usize, p: &TexelPoint) -> Vec4 {
        let reach = MAX_DISTANCE * baked.diagonal;
        let mut rng = Rng::new(0, AO_STREAM, i as u64);
        let (origin, dirs) = rays_off(baked, p, 1.0, &mut rng);
        let (mut open, mut cast) = (0u32, 0u32);
        for dir in dirs {
            cast += 1;
            open += u32::from(!self.rays.occluded_by(origin, dir, (0.0, reach), |_| true));
        }
        Vec3::splat(open as f32 / cast.max(1) as f32).extend(1.0)
    }

    /// Thickness of the texel at `p`: how far [`SAMPLES`] cosine rays cast into the
    /// mesh (around the inverted normal) travel before they leave it, averaged, as a
    /// share of [`MAX_DISTANCE`]; a ray that meets nothing counts the whole distance.
    /// 1 is thick, towards 0 thin.
    pub(super) fn thickness(&self, baked: &Baked, i: usize, p: &TexelPoint) -> Vec4 {
        let reach = MAX_DISTANCE * baked.diagonal;
        let mut rng = Rng::new(0, THICKNESS_STREAM, i as u64);
        let (origin, dirs) = rays_off(baked, p, -1.0, &mut rng);
        let (mut travelled, mut cast) = (0.0f32, 0u32);
        for dir in dirs {
            cast += 1;
            travelled += self.distance(origin, dir, reach);
        }
        Vec3::splat(travelled / (cast.max(1) as f32 * reach)).extend(1.0)
    }

    /// How far `origin + t * dir` goes before it meets a triangle, at most `reach`.
    fn distance(&self, origin: Vec3, dir: Vec3, reach: f32) -> f32 {
        let Some(hit) = self.rays.closest(origin, dir, 0.0, reach) else {
            return reach;
        };
        let t = &self.rays.tris[hit.tri];
        let point = t.a + t.e1 * hit.u + t.e2 * hit.v;
        point.distance(origin).min(reach)
    }
}

/// The origin and the [`SAMPLES`] cosine directions a texel casts, `side` 1 out of
/// the surface, -1 into it. They are drawn around the interpolated normal, and a
/// direction on the wrong side of the triangle's own face is dropped, so on a
/// smooth low-poly mesh no ray starts under the face it leaves.
fn rays_off<'r>(
    baked: &Baked,
    p: &TexelPoint,
    side: f32,
    rng: &'r mut Rng,
) -> (Vec3, impl Iterator<Item = Vec3> + 'r) {
    let face = baked.source.face_normal(p.first).unwrap_or(p.normal) * side;
    let around = p.normal * side;
    let origin = p.position + face * OFFSET * baked.diagonal;
    let dirs = (0..SAMPLES)
        .map(move |_| cosine_sample(around, rng))
        .filter(move |d| d.dot(face) > 0.0);
    (origin, dirs)
}
