use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

pub(in crate::lightmap) mod schedule;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pass {
    pub done: u32,
    pub passes: u32,
    pub samples: u32,
    pub view: u32,
    pub target: u32,
}

pub trait PassSink {
    fn wants(&self) -> bool {
        true
    }
    fn pass(&mut self, pass: Pass, maps: Vec<Lightmap>);
}

impl<F: FnMut(Pass, Vec<Lightmap>)> PassSink for F {
    fn pass(&mut self, pass: Pass, maps: Vec<Lightmap>) {
        self(pass, maps);
    }
}

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

pub(in crate::lightmap) fn progressive_on(
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
            if last {
                self.progress.pass_done(pass);
                note_buried(self.orphans.load(Ordering::Relaxed));
                return Some(maps.into_iter().flatten().collect());
            }
            if sink.wants() {
                let still = Step { seen: 0, rest: 0 };
                (work.done, work.step, work.show) = (done, still, true);
                let (shown, maps) = self.pass(going, &work)?;
                sink.pass(pass, maps.into_iter().flatten().collect());
                going = shown;
            }
            self.progress.pass_done(pass);
            items = going;
        }
        None
    }
}

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

struct Run<'a> {
    scene: &'a BakeScene,
    settings: &'a BakeSettings,
    progress: &'a BakeProgress,
    heads: &'a [(usize, u32)],
    seen: Option<&'a [Vec<bool>]>,
    threads: usize,
    layouts: Vec<Mutex<Option<Layout>>>,
    orphans: AtomicUsize,
}

impl Run<'_> {
    fn pass(&self, items: Charts, work: &Work) -> Option<(Charts, Vec<Option<Lightmap>>)> {
        let (heads, progress) = (self.heads, self.progress);
        let cleaner = Cleaner {
            denoiser: self.settings.denoiser,
            radius: self.settings.filter_radius,
            trained: progress.denoiser(),
        };
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
            progress.mesh_done(c);
            (Vec::new(), Some(map))
        };
        let out = pool::run_batched(items, self.threads, progress, &first, trace, finish)?;
        Some(out.into_iter().unzip())
    }
}
