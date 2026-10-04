//! The lightmap bake (#438): a [`BakeScene`] in, one [`Lightmap`] per receiving mesh
//! out. Deterministic: every texel draws its samples from its own seeded stream, so
//! the same scene, settings and seed give identical texels on any machine and with
//! any number of threads. The texels are split across the machine's cores.

use std::thread;

use glam::Vec3;

use super::bvh::{Bvh, Tri};
use super::filter::smooth;
use super::input::BakeScene;
use super::raster::{dilate, lightmap_size, rasterize, world_per_uv, TexelPoint};
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};

/// Rings of empty texels dilation fills around each chart.
const DILATE_PASSES: u32 = 3;

/// The bake's knobs, Unity's Lightmapping settings in miniature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeSettings {
    /// Lightmap texels per world unit (Unity's "Lightmap Resolution").
    pub texels_per_unit: f32,
    /// Paths traced per texel; more is smoother and slower.
    pub samples: u32,
    /// Surfaces a path bounces off before it stops.
    pub bounces: u32,
    /// The seed every texel's sample stream derives from.
    pub seed: u64,
    /// Largest lightmap edge, in texels (at most an atlas page, less its ring).
    pub max_resolution: u32,
    /// Radius, in texels, of the Gaussian that smooths the baked bounce (Unity's
    /// indirect filter); 0 leaves it raw. Baked direct light is never filtered.
    pub filter_radius: u32,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            texels_per_unit: 8.0,
            samples: 128,
            bounces: 3,
            seed: 0,
            max_resolution: 512,
            filter_radius: 3,
        }
    }
}

/// One mesh's baked lightmap: `size`² linear texels, row-major, each the `E / π`
/// value the forward shader multiplies by albedo (see `trace`).
#[derive(Clone, Debug, PartialEq)]
pub struct Lightmap {
    pub entity: u32,
    pub size: u32,
    pub texels: Vec<Vec3>,
}

/// Bake every mesh in `scene` that has a usable lightmap UV. Meshes without one still
/// occlude and bounce light; they just get no lightmap.
pub fn bake(scene: &BakeScene, settings: &BakeSettings) -> Vec<Lightmap> {
    let bvh = Bvh::build(triangles(scene));
    let tracer = Tracer { scene, bvh: &bvh };
    let mut out = Vec::new();
    // Every lightmap must fit an atlas page with its ring around it.
    let max = settings.max_resolution.min(super::atlas::MAX_PAGE - 2);
    for (index, mesh) in scene.meshes.iter().enumerate() {
        let Some(size) = lightmap_size(mesh, settings.texels_per_unit, max) else {
            continue;
        };
        let points = rasterize(mesh, size);
        let light = trace_all(&tracer, &points, index as u64, settings);
        // Texels inside other geometry are dropped here and filled by dilation below.
        let (points, light): (Vec<TexelPoint>, Vec<TexelLight>) = points
            .into_iter()
            .zip(light)
            .filter(|(_, l)| l.valid)
            .unzip();
        let indirect: Vec<Vec3> = light.iter().map(|l| l.indirect).collect();
        let texel_world = world_per_uv(mesh).unwrap_or(1.0) / size as f32;
        let radius = settings.filter_radius;
        let indirect = smooth(&points, &indirect, size, radius, texel_world);
        let mut texels = vec![Vec3::ZERO; (size * size) as usize];
        let mut filled = vec![false; texels.len()];
        for ((point, light), indirect) in points.iter().zip(light).zip(indirect) {
            texels[point.index] = indirect + light.direct;
            filled[point.index] = true;
        }
        dilate(&mut texels, &mut filled, size, DILATE_PASSES);
        out.push(Lightmap {
            entity: mesh.entity,
            size,
            texels,
        });
    }
    out
}

/// Every mesh's triangles, for the BVH.
fn triangles(scene: &BakeScene) -> Vec<Tri> {
    let mut tris = Vec::new();
    for (m, mesh) in scene.meshes.iter().enumerate() {
        for (k, t) in mesh.indices.chunks_exact(3).enumerate() {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
            tris.push(Tri::new(a, b, c, m as u32, (k * 3) as u32));
        }
    }
    tris
}

/// Trace `points` across the available cores, keeping their order. Each texel's
/// stream is seeded by `(seed, mesh, texel index)`, never by which thread runs it.
fn trace_all(
    tracer: &Tracer,
    points: &[TexelPoint],
    mesh: u64,
    s: &BakeSettings,
) -> Vec<TexelLight> {
    let one = |p: &TexelPoint| {
        let mut rng = Rng::new(s.seed, mesh, p.index as u64);
        tracer.texel(p.position, p.normal, s.samples, s.bounces, &mut rng)
    };
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = points.len().div_ceil(threads).max(1);
    thread::scope(|scope| {
        let workers: Vec<_> = points
            .chunks(chunk)
            .map(|run| scope.spawn(move || run.iter().map(one).collect::<Vec<_>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("lightmap worker panicked"))
            .collect()
    })
}
