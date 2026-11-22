use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

use super::input::BakeScene;
use super::progress::BakeProgress;
use super::raster::{
    chart_of_triangles, lightmap_size, rasterize, rasterize_counted, texel_charts, world_per_uv,
    TexelPoint,
};
use super::rays::{Rays, Tri};
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};
use crate::lightmap::max_map_edge;
use rusty_scene::lighting::settings::BakeSettings;

mod assemble;
mod denoise;
pub(super) mod direction;
mod map;
pub(crate) mod pool;
pub(super) mod progressive;
pub(super) mod view;

use assemble::assemble;
pub use denoise::LightmapDenoiser;
use denoise::{Chart, Cleaner};
pub use map::Lightmap;
pub use progressive::{bake_progressive, Pass, PassSink};
pub use view::BakeView;

/// Dense maps cost about 256 bytes a texel of the square, so two large charts
/// assembled at once on two workers would hold two of them.
pub const ASSEMBLE_ALONE: u64 = 512 * 512;

static ASSEMBLING: Mutex<()> = Mutex::new(());

pub fn bake(scene: &BakeScene, settings: &BakeSettings) -> Vec<Lightmap> {
    bake_with_progress(scene, settings, &BakeProgress::default()).unwrap_or_default()
}

pub fn bake_with_progress(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
) -> Option<Vec<Lightmap>> {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    bake_on(scene, settings, progress, threads)
}

/// A test hook: `dev::bake::lightmap`'s memory test bakes on a fixed number of workers.
pub fn bake_on(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    threads: usize,
) -> Option<Vec<Lightmap>> {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let (heads, counts) = counted(scene, settings);
    let texels = counts.iter().map(|&n| n as u64).sum();
    progress.set_total(texels, heads.len() as u64);
    let sizes = heads.iter().zip(&counts);
    progress.set_meshes(sizes.map(|((i, _), &n)| (scene.meshes[*i].entity, n as u64)));
    let trace = |c: usize, _: usize, batch: &[TexelPoint]| {
        let mesh = heads[c].0 as u64;
        let one = |p: &TexelPoint| texel(&tracer, settings, mesh, p);
        progress.batch_started(c);
        let traced = batch.iter().map(one).collect();
        progress.batch_finished(c, batch.len());
        traced
    };
    let make = |c: usize| rasterize_counted(&scene.meshes[heads[c].0], heads[c].1, counts[c]);
    let cleaner = Cleaner {
        denoiser: settings.denoiser,
        radius: settings.filter_radius,
        trained: progress.denoiser(),
    };
    let orphans = AtomicUsize::new(0);
    let finish = |c: usize, points: Vec<TexelPoint>, light: Vec<TexelLight>| {
        let head = heads[c];
        let shape = layout(scene, head, &points);
        let (map, lost) = lightmap(scene, (head, &shape), points, light, settings, cleaner);
        orphans.fetch_add(lost, Ordering::Relaxed);
        // Dropped before `mesh_done`, whose observer hands freed memory back to the system.
        drop(shape);
        progress.mesh_done(c);
        map
    };
    let maps = pool::run_made(&counts, make, threads, progress, trace, finish)?;
    note_buried(orphans.into_inner());
    Some(maps)
}

fn counted(scene: &BakeScene, settings: &BakeSettings) -> (Vec<(usize, u32)>, Vec<usize>) {
    let max = max_map_edge(settings.page());
    lightmapped(scene, settings, max)
        .map(|(index, size)| ((index, size), rasterize(&scene.meshes[index], size).len()))
        .unzip()
}

fn lightmapped<'a>(
    scene: &'a BakeScene,
    settings: &'a BakeSettings,
    max: u32,
) -> impl Iterator<Item = (usize, u32)> + 'a {
    scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, mesh)| mesh.receives_lightmap())
        .filter_map(move |(index, mesh)| {
            let density = settings.texels_per_unit * mesh.scale_in_lightmap;
            Some((index, lightmap_size(mesh, density, max)?))
        })
}

pub(super) type Receivers = (Vec<(usize, u32)>, Vec<Vec<TexelPoint>>);

pub(super) fn receivers(scene: &BakeScene, settings: &BakeSettings) -> Receivers {
    let max = max_map_edge(settings.page());
    lightmapped(scene, settings, max)
        .map(|(index, size)| ((index, size), rasterize(&scene.meshes[index], size)))
        .unzip()
}

pub(in crate::lightmap) fn texel(
    tracer: &Tracer,
    s: &BakeSettings,
    mesh: u64,
    p: &TexelPoint,
) -> TexelLight {
    let index = p.index as u64;
    let mut probes = Rng::probes(s.seed, mesh, index);
    if !tracer.valid(p.position, p.normal, s.backface_tolerance, &mut probes) {
        return TexelLight::INSIDE;
    }
    let mut rng = Rng::new(s.seed, mesh, index);
    let samples = (s.samples, s.direct_sample_count());
    tracer.texel(p.position, p.normal, samples, s.bounces, &mut rng)
}

fn note_buried(islands: usize) {
    if islands > 0 {
        log::info!(
            "[Lighting] lightmap bake: {islands} lightmap UV island(s) are fully covered by \
             other geometry (a face resting on a floor or against a neighbour) and take the \
             light of the islands beside them"
        );
    }
}

pub(super) struct Layout {
    labels: Vec<u32>,
    texel_world: f32,
}

pub(super) fn layout(
    scene: &BakeScene,
    (index, size): (usize, u32),
    points: &[TexelPoint],
) -> Layout {
    let mesh = &scene.meshes[index];
    let surface = mesh.surface();
    Layout {
        labels: texel_charts(points, &chart_of_triangles(&surface), size),
        texel_world: world_per_uv(&surface).unwrap_or(1.0) / size as f32,
    }
}

fn lightmap(
    scene: &BakeScene,
    ((index, size), layout): ((usize, u32), &Layout),
    points: Vec<TexelPoint>,
    light: Vec<TexelLight>,
    settings: &BakeSettings,
    cleaner: Cleaner,
) -> (Lightmap, usize) {
    let mesh = &scene.meshes[index];
    let valid = light.iter().filter(|l| l.valid).count();
    let (mut kept, mut lit) = (Vec::with_capacity(valid), Vec::with_capacity(valid));
    for (p, l) in points.into_iter().zip(light).filter(|(_, l)| l.valid) {
        kept.push(p);
        lit.push(l);
    }
    let (points, light) = (kept, lit);
    let masked = scene.shadowmask.any();
    let chart = Chart {
        points: &points,
        size,
        texel_world: layout.texel_world,
    };
    // Taken before the denoiser's budget (inside `LightmapDenoiser::denoise`), never while
    // holding it, so the two cannot deadlock.
    let _alone = (u64::from(size) * u64::from(size) > ASSEMBLE_ALONE)
        .then(|| ASSEMBLING.lock().unwrap_or_else(PoisonError::into_inner));
    let maps = assemble(chart, &layout.labels, &light, settings, cleaner, masked);
    let map = Lightmap::sparse(mesh.entity, size, maps.filled, maps.kept);
    (map, maps.orphans)
}

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
