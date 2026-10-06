//! The lightmap bake (#438): a [`BakeScene`] in, one [`Lightmap`] per receiving mesh
//! out. Deterministic: every texel draws its samples from its own seeded stream, so
//! the same scene, settings and seed give identical texels on any machine and with
//! any number of threads. The texels are split across the machine's cores, and a
//! [`BakeProgress`] can watch them go by or stop the bake (#808) without changing a
//! completed bake's bytes.

use std::thread;

use glam::Vec3;

use super::bvh::{Bvh, Tri};
use super::filter::smooth;
use super::input::BakeScene;
use super::progress::BakeProgress;
use super::raster::{dilate, lightmap_size, rasterize, world_per_uv, TexelPoint};
use super::rng::Rng;
use super::trace::{luminance, TexelLight, Tracer};

/// Rings of empty texels dilation fills around each chart.
const DILATE_PASSES: u32 = 3;

/// Texels a worker traces between progress reports and cancel checks: small enough
/// that a cancel lands at once, large enough that the shared counter stays cold.
const REPORT_EVERY: usize = 64;

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
    /// Also bake each texel's dominant incoming light direction (#810, Unity's
    /// *Directional* mode), so normal maps reshape baked light. Off saves the
    /// direction pages' memory; the colour lightmaps are the same either way.
    pub directional: bool,
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
            directional: true,
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
    /// Per texel (#810), the dominant direction its light arrives from, scaled by
    /// its directionality: length 1 when it all comes from one way, 0 when it comes
    /// evenly from everywhere. Empty when the bake was not [`BakeSettings::directional`].
    pub directions: Vec<Vec3>,
}

/// Bake every mesh in `scene` that has a usable lightmap UV. Meshes without one still
/// occlude and bounce light; they just get no lightmap.
pub fn bake(scene: &BakeScene, settings: &BakeSettings) -> Vec<Lightmap> {
    // A progress nobody holds is never cancelled, so this always runs to the end.
    bake_with_progress(scene, settings, &BakeProgress::default()).unwrap_or_default()
}

/// [`bake`], reporting texels traced into `progress` and stopping early when it is
/// cancelled. `None` means cancelled; a bake that finishes is byte-identical to
/// [`bake`]'s.
pub fn bake_with_progress(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
) -> Option<Vec<Lightmap>> {
    let bvh = Bvh::build(triangles(scene));
    let tracer = Tracer { scene, bvh: &bvh };
    // Every lightmap must fit an atlas page with its ring around it.
    let max = settings.max_resolution.min(super::atlas::MAX_PAGE - 2);
    // Rasterize everything first, so the total is known before the first texel.
    let charts: Vec<(usize, u32, Vec<TexelPoint>)> = scene
        .meshes
        .iter()
        .enumerate()
        .filter_map(|(index, mesh)| {
            let size = lightmap_size(mesh, settings.texels_per_unit, max)?;
            Some((index, size, rasterize(mesh, size)))
        })
        .collect();
    progress.set_total(charts.iter().map(|(_, _, p)| p.len() as u64).sum());
    let mut out = Vec::with_capacity(charts.len());
    for (index, size, points) in charts {
        let mesh = &scene.meshes[index];
        let light = trace_all(&tracer, &points, index as u64, settings, progress)?;
        // Texels inside other geometry are dropped here and filled by dilation below.
        let (points, light): (Vec<TexelPoint>, Vec<TexelLight>) = points
            .into_iter()
            .zip(light)
            .filter(|(_, l)| l.valid)
            .unzip();
        let texel_world = world_per_uv(mesh).unwrap_or(1.0) / size as f32;
        let (texels, directions) = assemble(&points, &light, size, texel_world, settings);
        out.push(Lightmap {
            entity: mesh.entity,
            size,
            texels,
            directions,
        });
    }
    Some(out)
}

/// One chart's traced texels as its `size`² lightmap and, for a directional bake,
/// its direction map: the bounce (and its directions) smoothed, the direct light
/// added, then both dilated past the chart's edge.
fn assemble(
    points: &[TexelPoint],
    light: &[TexelLight],
    size: u32,
    texel_world: f32,
    settings: &BakeSettings,
) -> (Vec<Vec3>, Vec<Vec3>) {
    let indirect: Vec<Vec3> = light.iter().map(|l| l.indirect).collect();
    let radius = settings.filter_radius;
    let indirect = smooth(points, &indirect, size, radius, texel_world);
    // The bounce's directions are smoothed by the same kernel as its colour, so
    // the direction page is no noisier than the lightmap it reshapes.
    let indirect_dir: Vec<Vec3> = match settings.directional {
        true => {
            let dirs: Vec<Vec3> = light.iter().map(|l| l.indirect_dir).collect();
            smooth(points, &dirs, size, radius, texel_world)
        }
        false => vec![Vec3::ZERO; points.len()],
    };
    let mut texels = vec![Vec3::ZERO; (size * size) as usize];
    let mut directions = vec![Vec3::ZERO; texels.len()];
    let mut filled = vec![false; texels.len()];
    for (k, (point, light)) in points.iter().zip(light).enumerate() {
        texels[point.index] = indirect[k] + light.direct;
        directions[point.index] = dominant(indirect[k], indirect_dir[k], light);
        filled[point.index] = true;
    }
    if settings.directional {
        dilate(&mut directions, &mut filled.clone(), size, DILATE_PASSES);
    } else {
        directions.clear();
    }
    dilate(&mut texels, &mut filled, size, DILATE_PASSES);
    (texels, directions)
}

/// A texel's dominant incoming direction scaled by its directionality: the
/// luminance-weighted mean of the directions its (smoothed) bounce and its direct
/// light arrived from. Black texels have none.
fn dominant(indirect: Vec3, indirect_dir: Vec3, light: &TexelLight) -> Vec3 {
    let weight = luminance(indirect + light.direct);
    if weight <= 0.0 {
        return Vec3::ZERO;
    }
    ((indirect_dir + light.direct_dir) / weight).clamp_length_max(1.0)
}

/// Every mesh's triangles, for the BVH.
pub(super) fn triangles(scene: &BakeScene) -> Vec<Tri> {
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
/// `None` when `progress` was cancelled before every texel was traced.
fn trace_all(
    tracer: &Tracer,
    points: &[TexelPoint],
    mesh: u64,
    s: &BakeSettings,
    progress: &BakeProgress,
) -> Option<Vec<TexelLight>> {
    let one = |p: &TexelPoint| {
        let mut rng = Rng::new(s.seed, mesh, p.index as u64);
        tracer.texel(p.position, p.normal, s.samples, s.bounces, &mut rng)
    };
    let run = |run: &[TexelPoint]| {
        let mut out = Vec::with_capacity(run.len());
        for batch in run.chunks(REPORT_EVERY) {
            if progress.is_cancelled() {
                break;
            }
            out.extend(batch.iter().map(one));
            progress.advance(batch.len() as u64);
        }
        out
    };
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = points.len().div_ceil(threads).max(1);
    let light: Vec<TexelLight> = thread::scope(|scope| {
        let workers: Vec<_> = points
            .chunks(chunk)
            .map(|chunk| scope.spawn(move || run(chunk)))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("lightmap worker panicked"))
            .collect()
    });
    (light.len() == points.len()).then_some(light)
}
