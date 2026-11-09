//! The progressive lightmap bake (#1970), Unity's Progressive Lightmapper: the
//! trace runs in passes over every texel of the scene, not mesh by mesh. Each pass
//! adds [`BakeSettings::samples_per_pass`] paths to every texel, and every pass
//! but the last hands the caller the lightmaps as they stand (accumulated, filled,
//! cleaned for display), so the editor can show the whole scene sharpening in place.
//! With a [`BakeView`], each pass traces what the camera sees first.
//!
//! **The final bytes are the one-shot bake's** ([`bake`](super::bake)) at the same
//! settings and seed: a texel keeps its one seeded stream and its sums between
//! passes, so its paths are drawn and added in the same order, and its direct
//! light comes from that stream after its last path, as one-shot. What a pass
//! shows is built on a copy: its direct light from a display stream of its own
//! (`Rng::preview`), its denoise on the means so far, and the last pass resolves and
//! cleans the full sums once. A cancelled bake returns `None`.
//!
//! **Memory:** per texel its point, stream, sums and shown direct light
//! (`Texel`, about 110 bytes), never per sample, in one copy: the pool hands each batch to its
//! worker to update in place (`pool::run_batched`). A pass adds, per chart until
//! it finishes, its texels' light and points, and the lightmaps it hands on.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use super::super::input::BakeScene;
use super::super::progress::BakeProgress;
use super::super::raster::TexelPoint;
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::super::trace::{Paths, Shown, TexelLight, Tracer};
use super::denoise::Cleaner;
use super::view::BakeView;
use super::{lightmap, note_buried, pool, receivers, triangles, BakeSettings, Lightmap};

/// A progressive bake's pass, as its caller hears of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pass {
    /// Passes finished, this one included.
    pub done: u32,
    /// Passes in all ([`BakeSettings::passes`]).
    pub passes: u32,
    /// Paths per texel traced so far.
    pub samples: u32,
    /// Paths per texel the bake traces in all ([`BakeSettings::samples`]).
    pub target: u32,
}

/// One texel between passes: where it is, its stream, its sums, whether it lies
/// inside geometry (#1949, decided on its first pass and never traced then), and
/// once a pass has shown it, what it shows.
#[derive(Debug)]
struct Texel {
    point: TexelPoint,
    rng: Rng,
    paths: Paths,
    inside: bool,
    shown: Option<Shown>,
}

/// Every chart's texels, cut into the pool's batches.
type Charts = Vec<Vec<Vec<Texel>>>;

/// Bake `scene` progressively on every core, handing `on_pass` each pass but the
/// last and its lightmaps, the batches `view` sees first in each pass. `None` when
/// `progress` was cancelled; a bake that finishes is byte-identical to `bake`'s.
pub fn bake_progressive(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    view: Option<BakeView>,
    on_pass: &mut dyn FnMut(Pass, Vec<Lightmap>),
) -> Option<Vec<Lightmap>> {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    progressive_on(scene, settings, progress, (view, threads), on_pass)
}

/// [`bake_progressive`] on `threads` workers.
pub(in crate::scene::lighting::lightmap) fn progressive_on(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    (view, threads): (Option<BakeView>, usize),
    on_pass: &mut dyn FnMut(Pass, Vec<Lightmap>),
) -> Option<Vec<Lightmap>> {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let (heads, points) = receivers(scene, settings);
    let passes = settings.passes();
    let seen = match view {
        Some(v) => Some(v.seen(&rays, &points, threads, progress)?),
        None => None,
    };
    announce(scene, settings, progress, (&heads, &points));
    let mut items = fresh(points, &heads, settings.seed);
    let run = Run {
        scene,
        settings,
        progress,
        heads: &heads,
        seen: seen.as_deref(),
        threads,
        orphans: AtomicUsize::new(0),
    };
    let mut done = 0;
    for pass in 0..passes {
        let count = settings
            .samples_per_pass
            .max(1)
            .min(settings.samples - done);
        let last = pass + 1 == passes;
        let step = Step {
            tracer: &tracer,
            settings,
            done,
            count,
            last,
        };
        let (going, maps) = run.pass(items, &step)?;
        done += count;
        progress.pass_done(u64::from(pass + 1), u64::from(done));
        if last {
            note_buried(run.orphans.into_inner());
            return Some(maps);
        }
        let pass = Pass {
            done: pass + 1,
            passes,
            samples: done,
            target: settings.samples,
        };
        on_pass(pass, maps);
        items = going;
    }
    // `passes` is at least 1, so the last pass has returned.
    None
}

/// Tell `progress` the bake's size before its first texel: its texels over every
/// pass, its meshes, its passes and samples, and an observer its charts.
fn announce(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    (heads, points): (&[(usize, u32)], &[Vec<TexelPoint>]),
) {
    let passes = u64::from(settings.passes());
    let texels: u64 = points.iter().map(|p| p.len() as u64).sum();
    progress.set_total(texels * passes, heads.len() as u64);
    progress.set_passes(passes, u64::from(settings.samples));
    let sizes = heads.iter().zip(points);
    progress.set_meshes(sizes.map(|((i, _), p)| (scene.meshes[*i].entity, p.len() as u64)));
}

/// Every chart's texels before the first pass, each with its own stream, in the
/// pool's batches.
fn fresh(points: Vec<Vec<TexelPoint>>, heads: &[(usize, u32)], seed: u64) -> Charts {
    let chart = |(c, chart): (usize, Vec<TexelPoint>)| {
        let mesh = heads[c].0 as u64;
        let texel = move |point: TexelPoint| Texel {
            rng: Rng::new(seed, mesh, point.index as u64),
            point,
            paths: Paths::default(),
            inside: false,
            shown: None,
        };
        pool::batches(chart.into_iter().map(texel))
    };
    points.into_iter().enumerate().map(chart).collect()
}

/// What every pass of one bake shares.
struct Run<'a> {
    scene: &'a BakeScene,
    settings: &'a BakeSettings,
    progress: &'a BakeProgress,
    heads: &'a [(usize, u32)],
    /// Per chart, per batch, whether the view sees it (Prioritize View).
    seen: Option<&'a [Vec<bool>]>,
    threads: usize,
    /// UV islands with no valid texel, counted on the last pass (#1949).
    orphans: AtomicUsize,
}

impl Run<'_> {
    /// One pass over every chart's `items` on the pool, the seen batches first,
    /// each texel updated in place: the texels the next pass goes on with (none
    /// after the last), and each chart's lightmap as it now stands (the finished
    /// one on the last pass). `None` when cancelled.
    fn pass(&self, items: Charts, step: &Step) -> Option<(Charts, Vec<Lightmap>)> {
        let (heads, progress) = (self.heads, self.progress);
        let cleaner = Cleaner {
            denoiser: self.settings.denoiser,
            radius: self.settings.filter_radius,
            trained: progress.denoiser(),
        };
        // A chart with no texel has no batch in `seen` (#1970's review).
        let first = |c: usize, b: usize| {
            self.seen
                .is_some_and(|s| s.get(c).and_then(|s| s.get(b)).copied().unwrap_or(false))
        };
        let trace = |c: usize, _: usize, batch: &mut [Texel]| {
            let mesh = heads[c].0 as u64;
            progress.batch_started(c);
            let light = batch.iter_mut().map(|t| step.texel(mesh, t)).collect();
            progress.batch_finished(c, batch.len());
            light
        };
        let finish = |c: usize, texels: Vec<Vec<Texel>>, light: Vec<TexelLight>| {
            let points = texels.iter().flatten().map(|t| t.point).collect();
            let (map, lost) = lightmap(self.scene, heads[c], points, light, self.settings, cleaner);
            if !step.last {
                return (texels, map);
            }
            self.orphans.fetch_add(lost, Ordering::Relaxed);
            progress.mesh_done();
            // The last pass frees each chart's state as it finishes.
            (Vec::new(), map)
        };
        let out = pool::run_batched(items, self.threads, progress, &first, trace, finish)?;
        Some(out.into_iter().unzip())
    }
}

/// One pass's work on a texel: `count` more paths after `done`, the last pass
/// resolving the texel.
struct Step<'a> {
    tracer: &'a Tracer<'a>,
    settings: &'a BakeSettings,
    done: u32,
    count: u32,
    last: bool,
}

impl Step<'_> {
    /// Texel `t` of receiving mesh `mesh` after this pass, updated in place, and
    /// its light: what it shows so far, or on the last pass its finished light.
    /// Its first pass decides its validity as the one-shot bake does
    /// (`super::texel`); a pass that shows it takes its direct light once, from
    /// its display stream, so a bake of one pass traces it only as one-shot does.
    fn texel(&self, mesh: u64, t: &mut Texel) -> TexelLight {
        let (s, tracer) = (self.settings, self.tracer);
        let (position, normal) = (t.point.position, t.point.normal);
        let index = t.point.index as u64;
        if self.done == 0 {
            let mut probes = Rng::probes(s.seed, mesh, index);
            t.inside = !tracer.valid(position, normal, s.backface_tolerance, &mut probes);
        }
        if t.inside {
            return TexelLight::INSIDE;
        }
        let work = (self.count, s.bounces);
        tracer.gather(position, normal, work, &mut t.rng, &mut t.paths);
        if self.last {
            return tracer.resolve(position, normal, s.samples, &t.paths, &mut t.rng);
        }
        let shown = t.shown.get_or_insert_with(|| {
            let mut display = Rng::preview(s.seed, mesh, index);
            tracer.shown(position, normal, s.samples, &mut display)
        });
        t.paths.light(self.done + self.count, shown)
    }
}
