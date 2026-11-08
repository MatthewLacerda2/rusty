//! The lightmap bake (#438): a [`BakeScene`] in, one [`Lightmap`] per receiving mesh
//! out. Deterministic: every texel draws its samples from its own seeded stream, so
//! the same scene, settings and seed give identical texels on any machine and with
//! any number of threads. Every mesh's texels go through one work pool across the
//! machine's cores (`pool`, #1656), and a [`BakeProgress`] can watch them go by or
//! stop the bake (#808) without changing a completed bake's bytes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use glam::{Vec3, Vec4};

use super::atlas::max_map_edge;
use super::input::BakeScene;
use super::progress::BakeProgress;
use super::raster::{
    chart_of_triangles, lightmap_size, rasterize, texel_charts, world_per_uv, TexelPoint,
};
use super::rays::{Rays, Tri};
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};

mod assemble;
mod denoise;
pub(super) mod direction;
pub(in crate::scene::lighting) mod pool;
mod settings;

use assemble::assemble;
use denoise::{Chart, Cleaner};
pub use denoise::{Denoiser, LightmapDenoiser};
pub use settings::BakeSettings;

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
        let one = |p: &TexelPoint| texel(&tracer, settings, mesh, p);
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
    let orphans = AtomicUsize::new(0);
    let finish = |c: usize, points: Vec<TexelPoint>, light: Vec<TexelLight>| {
        let (index, size) = heads[c];
        let (map, lost) = lightmap(scene, (index, size), points, light, settings, cleaner);
        orphans.fetch_add(lost, Ordering::Relaxed);
        progress.mesh_done();
        map
    };
    let maps = pool::run(points, threads, progress, trace, finish)?;
    warn_orphans(orphans.into_inner());
    Some(maps)
}

/// Texel `p` of receiving mesh `mesh` traced: its light from its own seeded stream,
/// and its validity from a stream of its own the sample count never moves (#1949).
fn texel(tracer: &Tracer, s: &BakeSettings, mesh: u64, p: &TexelPoint) -> TexelLight {
    let index = p.index as u64;
    let mut rng = Rng::new(s.seed, mesh, index);
    let mut light = tracer.texel(p.position, p.normal, s.samples, s.bounces, &mut rng);
    let mut probes = Rng::probes(s.seed, mesh, index);
    light.valid = tracer.valid(p.position, p.normal, s.backface_tolerance, &mut probes);
    light
}

/// The bake's one warning for UV islands with no valid texel (#1949).
fn warn_orphans(orphans: usize) {
    if orphans > 0 {
        log::warn!(
            "[Lighting] lightmap bake: {orphans} lightmap UV island(s) lie wholly inside \
             geometry; their texels were filled from the islands beside them"
        );
    }
}

/// One mesh's lightmap from its traced texels, and how many of its charts had no
/// valid texel (`assemble::ChartMaps::orphans`).
fn lightmap(
    scene: &BakeScene,
    (index, size): (usize, u32),
    points: Vec<TexelPoint>,
    light: Vec<TexelLight>,
    settings: &BakeSettings,
    cleaner: Cleaner,
) -> (Lightmap, usize) {
    let mesh = &scene.meshes[index];
    let labels = texel_charts(&points, &chart_of_triangles(&mesh.surface()), size);
    // Texels inside other geometry are dropped here and filled from their own
    // chart's valid texels in `assemble` (#1949).
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
    let maps = assemble(chart, &labels, &light, settings, cleaner, masked);
    let map = Lightmap {
        entity: mesh.entity,
        size,
        texels: maps.texels,
        directions: maps.directions,
        masks: maps.masks,
    };
    (map, maps.orphans)
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
