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
//! **Memory:** per texel its point, stream, sums and shown direct light, about 110
//! bytes, never per sample; a pass holds two copies of a chart's state until the
//! chart finishes.

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

/// One texel between passes: where it is, its stream, its sums, and once its
/// first pass has run, what it shows (`None` then means it lies inside geometry,
/// #1949, and is never traced).
#[derive(Clone, Debug)]
struct Texel {
    point: TexelPoint,
    rng: Rng,
    paths: Paths,
    shown: Option<Shown>,
}

/// A texel after a pass: going on, or (the last pass) its light resolved.
enum Traced {
    Going(Texel),
    Done(TexelLight),
}

/// Bake `scene` progressively on every core, telling `on_pass` each pass but the
/// last and its lightmaps, the batches `view` sees first in each pass. `None` when
/// `progress` was cancelled; a bake that finishes is byte-identical to `bake`'s.
pub fn bake_progressive(
    scene: &BakeScene,
    settings: &BakeSettings,
    progress: &BakeProgress,
    view: Option<BakeView>,
    on_pass: &mut dyn FnMut(Pass, &[Lightmap]),
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
    on_pass: &mut dyn FnMut(Pass, &[Lightmap]),
) -> Option<Vec<Lightmap>> {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let (heads, points) = receivers(scene, settings);
    let passes = settings.passes();
    let texels: u64 = points.iter().map(|p| p.len() as u64).sum();
    progress.set_total(texels * u64::from(passes), heads.len() as u64);
    progress.set_passes(u64::from(passes), u64::from(settings.samples));
    let sizes = heads.iter().zip(&points);
    progress.set_meshes(sizes.map(|((i, _), p)| (scene.meshes[*i].entity, p.len() as u64)));
    let seen = view.map(|v| v.seen(&rays, &points, threads));
    let first = |c: usize, b: usize| seen.as_ref().is_some_and(|s| s[c][b]);
    let mut items: Vec<Vec<Texel>> = (points.into_iter().enumerate())
        .map(|(c, chart)| {
            let mesh = heads[c].0 as u64;
            let rng = |p: &TexelPoint| Rng::new(settings.seed, mesh, p.index as u64);
            let texel = |point| Texel {
                rng: rng(&point),
                point,
                paths: Paths::default(),
                shown: None,
            };
            chart.into_iter().map(texel).collect()
        })
        .collect();
    let cleaner = Cleaner {
        denoiser: settings.denoiser,
        radius: settings.filter_radius,
        trained: progress.denoiser(),
    };
    let orphans = AtomicUsize::new(0);
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
        let trace = |c: usize, _: usize, batch: &[Texel]| {
            let mesh = heads[c].0 as u64;
            progress.batch_started(c);
            let traced = batch.iter().map(|t| step.texel(mesh, t)).collect();
            progress.batch_finished(c, batch.len());
            traced
        };
        let finish = |c: usize, before: Vec<Texel>, traced: Vec<Traced>| {
            let points: Vec<TexelPoint> = before.iter().map(|t| t.point).collect();
            let (light, going) = step.light(traced);
            let (map, lost) = lightmap(scene, heads[c], points, light, settings, cleaner);
            if last {
                orphans.fetch_add(lost, Ordering::Relaxed);
                progress.mesh_done();
            }
            (going, map)
        };
        let out = pool::run_first(items, threads, progress, &first, trace, finish)?;
        done += count;
        progress.pass_done(u64::from(pass + 1), u64::from(done));
        let (going, maps): (Vec<Vec<Texel>>, Vec<Lightmap>) = out.into_iter().unzip();
        if last {
            note_buried(orphans.into_inner());
            return Some(maps);
        }
        let pass = Pass {
            done: pass + 1,
            passes,
            samples: done,
            target: settings.samples,
        };
        on_pass(pass, &maps);
        items = going;
    }
    // `passes` is at least 1, so the last pass has returned.
    None
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
    /// Texel `t` of receiving mesh `mesh` after this pass. Its first pass decides
    /// its validity as the one-shot bake does (`super::texel`) and takes what it
    /// shows from its display stream.
    fn texel(&self, mesh: u64, t: &Texel) -> Traced {
        let (s, tracer) = (self.settings, self.tracer);
        let mut t = t.clone();
        let (p, index) = (&t.point, t.point.index as u64);
        if self.done == 0 {
            let mut probes = Rng::probes(s.seed, mesh, index);
            if tracer.valid(p.position, p.normal, s.backface_tolerance, &mut probes) {
                let mut shown = Rng::preview(s.seed, mesh, index);
                t.shown = Some(tracer.shown(p.position, p.normal, s.samples, &mut shown));
            }
        }
        if t.shown.is_none() {
            return match self.last {
                true => Traced::Done(TexelLight::INSIDE),
                false => Traced::Going(t),
            };
        }
        let (position, normal) = (p.position, p.normal);
        let work = (self.count, s.bounces);
        tracer.gather(position, normal, work, &mut t.rng, &mut t.paths);
        match self.last {
            true => Traced::Done(tracer.resolve(position, normal, s.samples, &t.paths, &mut t.rng)),
            false => Traced::Going(t),
        }
    }

    /// A chart's traced texels as the light its lightmap is built from, and the
    /// texels the next pass goes on with (none after the last).
    fn light(&self, traced: Vec<Traced>) -> (Vec<TexelLight>, Vec<Texel>) {
        let samples = self.done + self.count;
        let mut going = Vec::with_capacity(if self.last { 0 } else { traced.len() });
        let light = traced
            .into_iter()
            .map(|t| match t {
                Traced::Done(light) => light,
                Traced::Going(t) => {
                    let light = match &t.shown {
                        Some(shown) => t.paths.light(samples, shown),
                        None => TexelLight::INSIDE,
                    };
                    going.push(t);
                    light
                }
            })
            .collect();
        (light, going)
    }
}
