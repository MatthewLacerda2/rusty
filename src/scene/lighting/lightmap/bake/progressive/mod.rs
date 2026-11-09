//! The progressive lightmap bake (#1970), Unity's Progressive Lightmapper: the
//! trace runs in passes over every texel of the scene, not mesh by mesh. Each pass
//! adds [`BakeSettings::samples_per_pass`] paths to every texel, and every pass
//! but the last hands the caller the lightmaps as they stand (accumulated, filled,
//! cleaned for display), so the editor can show the whole scene sharpening in place.
//! With a [`BakeView`] the camera's texels converge first: after the whole
//! scene's first pass they get all their other passes, then the rest get theirs
//! (`schedule`), so a bake that splits that way makes more steps than passes.
//! Once a step is traced, a display pass builds its lightmaps from the texels as
//! they stand, unless the [`PassSink`] is still busy with the last ones it took.
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
//! (`Texel`, 112 bytes), never per sample, in one copy: the pool hands each batch
//! to its worker to update in place (`pool::run_batched`), and each chart's
//! [`Layout`] once. A step whose lightmaps are built adds, per chart until it
//! finishes, its texels' light and points, and the lightmaps it hands on.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

pub(in crate::scene::lighting::lightmap) mod schedule;
mod texel;

use super::super::input::BakeScene;
use super::super::progress::BakeProgress;
use super::super::raster::TexelPoint;
use super::super::rays::Rays;
use super::super::trace::{TexelLight, Tracer};
use super::denoise::Cleaner;
use super::view::BakeView;
use super::{
    layout, lightmap, note_buried, pool, receivers, triangles, BakeSettings, Layout, Lightmap,
};
use schedule::{steps, Step};
use texel::{fresh, Charts, Texel, Work};

/// A progressive bake's step, as its caller hears of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pass {
    /// Steps finished, this one included.
    pub done: u32,
    /// Steps in all: [`BakeSettings::passes`], or with a view that sees part of
    /// the scene, its passes after the first once for the view and once for the
    /// rest (`schedule`).
    pub passes: u32,
    /// Paths per texel every texel has so far.
    pub samples: u32,
    /// Paths per texel the texels the view sees have so far: ahead of `samples`
    /// while the view converges first, equal otherwise.
    pub view: u32,
    /// Paths per texel the bake traces in all ([`BakeSettings::samples`]).
    pub target: u32,
}

/// Who hears a progressive bake's steps: whether it wants the lightmaps of the
/// step just traced (built only then, #1970's review), and each it wanted.
pub trait PassSink {
    /// Whether to build the lightmaps of the step just traced; a sink still busy
    /// with the last ones says no, and the bake traces on.
    fn wants(&self) -> bool {
        true
    }
    /// A step's lightmaps as they stand.
    fn pass(&mut self, pass: Pass, maps: Vec<Lightmap>);
}

impl<F: FnMut(Pass, Vec<Lightmap>)> PassSink for F {
    fn pass(&mut self, pass: Pass, maps: Vec<Lightmap>) {
        self(pass, maps);
    }
}

/// Bake `scene` progressively on every core, handing `sink` every step but the
/// last and its lightmaps when it wants them, what `view` sees converging first.
/// `None` when `progress` was cancelled; a bake that finishes is byte-identical
/// to `bake`'s.
pub fn bake_progressive(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    view: Option<BakeView>,
    sink: &mut dyn PassSink,
) -> Option<Vec<Lightmap>> {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    progressive_on(scene, settings, progress, (view, threads), sink)
}

/// [`bake_progressive`] on `threads` workers.
pub(in crate::scene::lighting::lightmap) fn progressive_on(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    (view, threads): (Option<BakeView>, usize),
    sink: &mut dyn PassSink,
) -> Option<Vec<Lightmap>> {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let (heads, points) = receivers(scene, settings);
    let seen = match view {
        Some(v) => Some(v.seen(&rays, &points, threads, progress)?),
        None => None,
    };
    let split = seen.as_ref().is_some_and(|s| {
        let all = s.iter().flatten();
        all.clone().any(|&b| b) && all.into_iter().any(|&b| !b)
    });
    let steps = steps(settings.samples, settings.samples_per_pass, split);
    announce(scene, settings, progress, (&heads, &points), steps.len());
    let layouts = heads.iter().map(|_| Mutex::new(None)).collect();
    let items = fresh(points, &heads, settings.seed);
    let run = Run {
        scene,
        settings,
        progress,
        heads: &heads,
        seen: seen.as_deref(),
        threads,
        layouts,
        orphans: AtomicUsize::new(0),
    };
    run.steps(items, &steps, &tracer, sink)
}

impl Run<'_> {
    /// Every one of `steps` over `items`, `sink` hearing each but the last that
    /// it wants: the finished lightmaps, or `None` when cancelled.
    fn steps(
        &self,
        mut items: Charts,
        steps: &[Step],
        tracer: &Tracer,
        sink: &mut dyn PassSink,
    ) -> Option<Vec<Lightmap>> {
        let mut done = Step { seen: 0, rest: 0 };
        for (k, &step) in steps.iter().enumerate() {
            let last = k + 1 == steps.len();
            let mut work = Work {
                tracer,
                settings: self.settings,
                done,
                step,
                last,
                show: false,
            };
            let (mut going, maps) = self.pass(items, &work)?;
            done = Step {
                seen: done.seen + step.seen,
                rest: done.rest + step.rest,
            };
            let pass = Pass {
                done: k as u32 + 1,
                passes: steps.len() as u32,
                samples: done.rest.min(done.seen),
                view: done.seen,
                target: self.settings.samples,
            };
            self.progress.pass_done(pass);
            if last {
                note_buried(self.orphans.load(Ordering::Relaxed));
                return Some(maps.into_iter().flatten().collect());
            }
            // Asked once the step is traced, so a sink that finished the last
            // lightmaps meanwhile gets these: a display pass over the texels as they
            // stand, tracing nothing.
            if sink.wants() {
                let still = Step { seen: 0, rest: 0 };
                (work.done, work.step, work.show) = (done, still, true);
                let (shown, maps) = self.pass(going, &work)?;
                sink.pass(pass, maps.into_iter().flatten().collect());
                going = shown;
            }
            items = going;
        }
        // There is at least one step, so the last has returned.
        None
    }
}

/// Tell `progress` the bake's size before its first texel: its texels over every
/// pass, its meshes, its `steps` and samples, and an observer its charts.
fn announce(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    (heads, points): (&[(usize, u32)], &[Vec<TexelPoint>]),
    steps: usize,
) {
    let passes = u64::from(settings.passes());
    let texels: u64 = points.iter().map(|p| p.len() as u64).sum();
    progress.set_total(texels * passes, heads.len() as u64);
    progress.set_passes(steps as u64, u64::from(settings.samples));
    let sizes = heads.iter().zip(points);
    progress.set_meshes(sizes.map(|((i, _), p)| (scene.meshes[*i].entity, p.len() as u64)));
}

/// What every step of one bake shares.
struct Run<'a> {
    scene: &'a BakeScene,
    settings: &'a BakeSettings,
    progress: &'a BakeProgress,
    heads: &'a [(usize, u32)],
    /// Per chart, per batch, whether the view sees it (Prioritize View).
    seen: Option<&'a [Vec<bool>]>,
    threads: usize,
    /// Per chart, its layout once a step has built its lightmap; freed on the last.
    layouts: Vec<Mutex<Option<Layout>>>,
    /// UV islands with no valid texel, counted on the last pass (#1949).
    orphans: AtomicUsize,
}

impl Run<'_> {
    /// One step over every chart's `items` on the pool, the seen batches queued first,
    /// each texel updated in place: the texels the next step goes on with (none
    /// after the last), and each chart's lightmap as it now stands when the step
    /// shows (the finished one on the last step). `None` when cancelled.
    fn pass(&self, items: Charts, work: &Work) -> Option<(Charts, Vec<Option<Lightmap>>)> {
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
        let trace = |c: usize, b: usize, batch: &mut [Texel]| {
            let (mesh, seen) = (heads[c].0 as u64, first(c, b));
            let traced = if work.step.of(seen) > 0 {
                batch.len()
            } else {
                0
            };
            if traced > 0 {
                progress.batch_started(c);
            }
            let light = batch
                .iter_mut()
                .filter_map(|t| work.texel(mesh, seen, t))
                .collect();
            if traced > 0 {
                progress.batch_finished(c, batch.len());
            }
            (light, traced as u64)
        };
        let finish = |c: usize, texels: Vec<Vec<Texel>>, light: Vec<TexelLight>| {
            if !work.show && !work.last {
                return (texels, None);
            }
            let points: Vec<TexelPoint> = texels.iter().flatten().map(|t| t.point).collect();
            let mut slot = self.layouts[c]
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let shape = slot.get_or_insert_with(|| layout(self.scene, heads[c], &points));
            let at = (heads[c], &*shape);
            let (map, lost) = lightmap(self.scene, at, points, light, self.settings, cleaner);
            if !work.last {
                return (texels, Some(map));
            }
            *slot = None;
            self.orphans.fetch_add(lost, Ordering::Relaxed);
            progress.mesh_done();
            // The last step frees each chart's state as it finishes.
            (Vec::new(), Some(map))
        };
        let out = pool::run_batched(items, self.threads, progress, &first, trace, finish)?;
        Some(out.into_iter().unzip())
    }
}
