//! The lightmap bake (#438): a [`BakeScene`] in, one [`Lightmap`] per receiving mesh
//! out. Deterministic: every texel draws its samples from its own seeded stream, so
//! the same scene, settings and seed give identical texels on any machine and with
//! any number of threads. Every mesh's texels go through one work pool across the
//! machine's cores (`pool`, #1656), and a [`BakeProgress`] can watch them go by or
//! stop the bake (#808) without changing a completed bake's bytes.

use std::ops::RangeInclusive;
use std::thread;

use glam::Vec3;

use super::filter::smooth;
use super::input::BakeScene;
use super::progress::BakeProgress;
use super::raster::{dilate, lightmap_size, rasterize, world_per_uv, TexelPoint};
use super::rays::{Rays, Tri};
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};
use crate::core::color::luminance;

pub(super) mod pool;

/// Rings of empty texels dilation fills around each chart.
const DILATE_PASSES: u32 = 3;

/// The bake's knobs, Unity's Lightmapping settings in miniature. The scene stores
/// them in its lighting settings (#832); a field a scene file omits takes its default.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
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

impl BakeSettings {
    /// The texel densities a bake takes (Unity's Lightmap Resolution, in miniature).
    pub const TEXELS_PER_UNIT: RangeInclusive<f32> = 0.25..=64.0;
    /// The samples per texel a bake takes.
    pub const SAMPLES: RangeInclusive<u32> = 1..=4096;
    /// The bounces a bake takes.
    pub const BOUNCES: RangeInclusive<u32> = 1..=16;

    /// These settings with each authored knob clamped into its range: the one place
    /// those limits live, applied by `Lighting.SetSettings` / `BakeLightmaps` and
    /// spanned by the editor's fields (#993). A non-finite density takes the default.
    pub fn sanitized(self) -> Self {
        let texels = match self.texels_per_unit.is_finite() {
            true => self.texels_per_unit,
            false => Self::default().texels_per_unit,
        };
        let clamp = |v: u32, r: RangeInclusive<u32>| v.clamp(*r.start(), *r.end());
        Self {
            texels_per_unit: texels
                .clamp(*Self::TEXELS_PER_UNIT.start(), *Self::TEXELS_PER_UNIT.end()),
            samples: clamp(self.samples, Self::SAMPLES),
            bounces: clamp(self.bounces, Self::BOUNCES),
            ..self
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

/// Bake every mesh in `scene` that has a usable lightmap UV and a camera sees (a
/// `ShadowsOnly` one gets none, #1571). Meshes without one still occlude and bounce
/// light; they just get no lightmap.
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
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    bake_on(scene, settings, progress, threads)
}

/// [`bake_with_progress`] on `threads` workers: one pool over every mesh's texels
/// (#1656). Each texel's stream is seeded by `(seed, mesh, texel index)`, never by
/// which worker runs it, so the thread count never shows in the result.
pub(super) fn bake_on(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    threads: usize,
) -> Option<Vec<Lightmap>> {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer { scene, rays: &rays };
    // Every lightmap must fit an atlas page with its ring around it.
    let max = settings.max_resolution.min(super::atlas::MAX_PAGE - 2);
    // Rasterize everything first, so the total is known before the first texel.
    let charts: Vec<(usize, u32, Vec<TexelPoint>)> = scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, mesh)| mesh.cast_shadows.is_visible())
        .filter_map(|(index, mesh)| {
            let size = lightmap_size(mesh, settings.texels_per_unit, max)?;
            Some((index, size, rasterize(mesh, size)))
        })
        .collect();
    progress.set_total(charts.iter().map(|(_, _, p)| p.len() as u64).sum());
    let points: Vec<&[TexelPoint]> = charts.iter().map(|(_, _, p)| p.as_slice()).collect();
    let trace = |c: usize, _: usize, batch: &[TexelPoint]| {
        let mesh = charts[c].0 as u64;
        let one = |p: &TexelPoint| {
            let mut rng = Rng::new(settings.seed, mesh, p.index as u64);
            let s = settings;
            tracer.texel(p.position, p.normal, s.samples, s.bounces, &mut rng)
        };
        batch.iter().map(one).collect()
    };
    let finish = |c: usize, light: Vec<TexelLight>| {
        let (index, size, ref points) = charts[c];
        lightmap(scene, index, size, points, light, settings)
    };
    pool::run(&points, threads, progress, trace, finish)
}

/// One mesh's lightmap from its traced texels.
fn lightmap(
    scene: &BakeScene,
    index: usize,
    size: u32,
    points: &[TexelPoint],
    light: Vec<TexelLight>,
    settings: &BakeSettings,
) -> Lightmap {
    let mesh = &scene.meshes[index];
    // Texels inside other geometry are dropped here and filled by dilation below.
    let (points, light): (Vec<TexelPoint>, Vec<TexelLight>) = points
        .iter()
        .copied()
        .zip(light)
        .filter(|(_, l)| l.valid)
        .unzip();
    let texel_world = world_per_uv(mesh).unwrap_or(1.0) / size as f32;
    let (texels, directions) = assemble(&points, &light, size, texel_world, settings);
    Lightmap {
        entity: mesh.entity,
        size,
        texels,
        directions,
    }
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

/// Every mesh's occluding triangles, for the BVH: a Transparent run lets light
/// through and bounces none (#1290). The mesh's *Cast Shadows* says who meets the
/// rest (#1571): bounce rays see all but a `ShadowsOnly` mesh's, shadow rays stop at
/// all but an `Off` one's.
pub(super) fn triangles(scene: &BakeScene) -> Vec<Tri> {
    let mut tris = Vec::new();
    for (m, mesh) in scene.meshes.iter().enumerate() {
        let (seen, blocks) = (mesh.cast_shadows.is_visible(), mesh.cast_shadows.casts());
        for (k, t) in mesh.indices.chunks_exact(3).enumerate() {
            if !mesh.run_at((k * 3) as u32).occludes {
                continue;
            }
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
            tris.push(Tri::new(a, b, c, m as u32, (k * 3) as u32).with(seen, blocks));
        }
    }
    tris
}
