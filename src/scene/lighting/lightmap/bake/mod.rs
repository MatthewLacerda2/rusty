//! The lightmap bake (#438): a [`BakeScene`] in, one [`Lightmap`] per receiving mesh
//! out. Deterministic: every texel draws its samples from its own seeded stream, so
//! the same scene, settings and seed give identical texels on any machine and with
//! any number of threads. Every mesh's texels go through one work pool across the
//! machine's cores (`pool`, #1656), and a [`BakeProgress`] can watch them go by or
//! stop the bake (#808) without changing a completed bake's bytes.

use std::ops::RangeInclusive;
use std::thread;

use glam::{Vec3, Vec4};

use super::atlas::{max_map_edge, page_edge, MAX_PAGE, MIN_PAGE};
use super::input::BakeScene;
use super::progress::BakeProgress;
use super::raster::{lightmap_size, rasterize, world_per_uv, TexelPoint};
use super::rays::{Rays, Tri};
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};

mod assemble;
mod denoise;
pub(super) mod direction;
pub(in crate::scene::lighting) mod pool;

use assemble::assemble;
use denoise::{Chart, Cleaner};
pub use denoise::{Denoiser, LightmapDenoiser};

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
    /// Unity's *Max Lightmap Size* (#1704): the atlas page's edge, in texels, a
    /// power of two in [`Self::MAX_LIGHTMAP_SIZE`]. A mesh's lightmap follows its
    /// area × `texels_per_unit` up to the largest map such a page holds
    /// ([`max_map_edge`]); the packer picks the smallest page that holds the bake,
    /// up to this edge, then adds pages. Read through [`Self::page`].
    pub max_lightmap_size: u32,
    /// What cleans the baked bounce (#1884, Unity's *Denoiser* and *Filter*): OIDN
    /// then the Gaussian below, the Gaussian alone, or nothing. Baked direct light
    /// is never cleaned.
    pub denoiser: Denoiser,
    /// Radius, in texels, of the Gaussian that smooths the baked bounce (Unity's
    /// indirect filter): alone under [`Denoiser::Filter`] or where OIDN cannot run,
    /// after OIDN under `Oidn`; 0 leaves it as traced or as OIDN gave it.
    pub filter_radius: u32,
    /// Also bake each texel's dominant incoming light direction and its `Baked`
    /// lamps' share (#810, Unity's *Directional* mode; #1695), so normal maps
    /// reshape baked light and `Baked` lamps give a lightmapped surface one
    /// specular highlight. Off saves the direction pages' memory; the colour
    /// lightmaps are the same either way.
    pub directional: bool,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            texels_per_unit: 8.0,
            samples: Self::DEFAULT_SAMPLES,
            bounces: 3,
            seed: 0,
            max_lightmap_size: 1024,
            denoiser: Denoiser::Oidn,
            filter_radius: 3,
            directional: true,
        }
    }
}

impl BakeSettings {
    /// The default paths per texel: the fewest at which an OIDN bake (OIDN, then
    /// the Gaussian) is as close to a converged one as the Gaussian alone was at
    /// its old default of 128, within a point of mean relative error, in about 60%
    /// of the time (#1884, `dev::bake::denoise::quality_tests`; 32 missed by 1.3 to
    /// 1.7 points). A bake that falls back to the Gaussian alone is blurrier and
    /// blotchier at this count.
    pub const DEFAULT_SAMPLES: u32 = 64;
    /// The texel densities a bake takes (Unity's Lightmap Resolution, in miniature).
    pub const TEXELS_PER_UNIT: RangeInclusive<f32> = 0.25..=64.0;
    /// The samples per texel a bake takes.
    pub const SAMPLES: RangeInclusive<u32> = 1..=4096;
    /// The bounces a bake takes.
    pub const BOUNCES: RangeInclusive<u32> = 1..=16;
    /// The page edges a bake takes (Unity's 32–4096), each a power of two.
    pub const MAX_LIGHTMAP_SIZE: RangeInclusive<u32> = MIN_PAGE..=MAX_PAGE;

    /// The atlas page edge this bake packs into: [`Self::max_lightmap_size`] held
    /// to a power of two in [`Self::MAX_LIGHTMAP_SIZE`] (a hand-edited file may
    /// hold anything).
    pub fn page(&self) -> u32 {
        page_edge(self.max_lightmap_size)
    }

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
            max_lightmap_size: self.page(),
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
    /// Per texel (#810), `xyz` the dominant direction its light arrives from, scaled
    /// by its directionality (length 1 when it all comes from one way, 0 when it comes
    /// evenly from everywhere), and `w` its lamp share (#1695: how much of it is
    /// `Baked` lamps' direct light from one way, `direction::direction_texel`). Empty
    /// when the bake was not [`BakeSettings::directional`].
    pub directions: Vec<Vec4>,
    /// Per texel (#1695), its shadowmask: per channel the static occlusion of the
    /// `Mixed` light on it (`trace::Tracer::mask`). Empty unless the bake gave a
    /// light a channel (`BakeScene::shadowmask`).
    pub masks: Vec<Vec4>,
}

/// Bake every mesh in `scene` that has a usable lightmap UV and receives a lightmap
/// ([`BakeMesh::receives_lightmap`](super::input::BakeMesh::receives_lightmap): a `ShadowsOnly` one gets none, #1571, nor one
/// at Scale In Lightmap 0, #1704), its texel density scaled by its Scale In
/// Lightmap. Meshes without one still occlude and bounce light; they just get no
/// lightmap.
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
    let tracer = Tracer::new(scene, &rays);
    // Every lightmap must fit a page with its ring around it.
    let max = max_map_edge(settings.page());
    // Rasterize everything first, so the total is known before the first texel.
    let charts: Vec<(usize, u32, Vec<TexelPoint>)> = scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, mesh)| mesh.receives_lightmap())
        .filter_map(|(index, mesh)| {
            let density = settings.texels_per_unit * mesh.scale_in_lightmap;
            let size = lightmap_size(mesh, density, max)?;
            Some((index, size, rasterize(mesh, size)))
        })
        .collect();
    let texels = charts.iter().map(|(_, _, p)| p.len() as u64).sum();
    progress.set_total(texels, charts.len() as u64);
    let (heads, points): (Vec<(usize, u32)>, Vec<Vec<TexelPoint>>) = charts
        .into_iter()
        .map(|(i, size, p)| ((i, size), p))
        .unzip();
    let sizes = heads.iter().zip(&points);
    progress.set_meshes(sizes.map(|((i, _), p)| (scene.meshes[*i].entity, p.len() as u64)));
    let trace = |c: usize, _: usize, batch: &[TexelPoint]| {
        let mesh = heads[c].0 as u64;
        let one = |p: &TexelPoint| {
            let mut rng = Rng::new(settings.seed, mesh, p.index as u64);
            let s = settings;
            tracer.texel(p.position, p.normal, s.samples, s.bounces, &mut rng)
        };
        // An observed bake (#1891) says each batch's mesh; it never touches a texel.
        progress.batch_started(c);
        let traced = batch.iter().map(one).collect();
        progress.batch_finished(c, batch.len());
        traced
    };
    // The pool hands each chart its points back once traced, so they die with it.
    let cleaner = Cleaner {
        denoiser: settings.denoiser,
        radius: settings.filter_radius,
        trained: progress.denoiser(),
    };
    let finish = |c: usize, points: Vec<TexelPoint>, light: Vec<TexelLight>| {
        let (index, size) = heads[c];
        let map = lightmap(scene, (index, size), points, light, settings, cleaner);
        progress.mesh_done();
        map
    };
    pool::run(points, threads, progress, trace, finish)
}

/// One mesh's lightmap from its traced texels.
fn lightmap(
    scene: &BakeScene,
    (index, size): (usize, u32),
    points: Vec<TexelPoint>,
    light: Vec<TexelLight>,
    settings: &BakeSettings,
    cleaner: Cleaner,
) -> Lightmap {
    let mesh = &scene.meshes[index];
    // Texels inside other geometry are dropped here and filled by dilation below.
    let (points, light): (Vec<TexelPoint>, Vec<TexelLight>) = points
        .into_iter()
        .zip(light)
        .filter(|(_, l)| l.valid)
        .unzip();
    let texel_world = world_per_uv(&mesh.surface()).unwrap_or(1.0) / size as f32;
    let masked = scene.shadowmask.any();
    let chart = Chart {
        points: &points,
        size,
        texel_world,
    };
    let maps = assemble(chart, &light, settings, cleaner, masked);
    Lightmap {
        entity: mesh.entity,
        size,
        texels: maps.texels,
        directions: maps.directions,
        masks: maps.masks,
    }
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
