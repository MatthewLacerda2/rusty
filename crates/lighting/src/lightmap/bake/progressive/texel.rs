//! A progressive bake's texels between steps (#1970) and one step's work on each.

use super::super::super::raster::TexelPoint;
use super::super::super::rng::Rng;
use super::super::super::trace::{Paths, Shown, TexelLight, Tracer};
use super::super::pool;
use super::schedule::Step;
use rusty_scene::lighting::settings::BakeSettings;

/// One texel between passes: where it is, its stream, its sums, whether it lies
/// inside geometry (#1949, decided on its first pass and never traced then), and
/// once a pass has shown it, what it shows.
#[derive(Debug)]
pub(super) struct Texel {
    pub point: TexelPoint,
    rng: Rng,
    paths: Paths,
    inside: bool,
    shown: Option<Shown>,
}

/// Every chart's texels, cut into the pool's batches.
pub(super) type Charts = Vec<Vec<Vec<Texel>>>;

/// Every chart's texels before the first pass, each with its own stream, in the
/// pool's batches.
pub(super) fn fresh(points: Vec<Vec<TexelPoint>>, heads: &[(usize, u32)], seed: u64) -> Charts {
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

/// One step's work on a texel: `step`'s paths after `done`, the last step
/// resolving every texel, a step that shows giving each its light so far.
pub(super) struct Work<'a> {
    pub tracer: &'a Tracer<'a>,
    pub settings: &'a BakeSettings,
    pub done: Step,
    pub step: Step,
    pub last: bool,
    pub show: bool,
}

impl Work<'_> {
    /// Texel `t` of receiving mesh `mesh` (in the view's group when `seen`) after
    /// this step, updated in place, and its light when the step needs it: what it
    /// shows so far, or on the last step its finished light. Its first pass
    /// decides its validity as the one-shot bake does (`super::texel`); a step that
    /// shows it takes its direct light once, from its display stream, so a bake of
    /// one pass traces it only as one-shot does. A texel whose paths were all in
    /// before the last step is resolved there: nothing drew from its stream since.
    pub fn texel(&self, mesh: u64, seen: bool, t: &mut Texel) -> Option<TexelLight> {
        let (s, tracer) = (self.settings, self.tracer);
        let (position, normal) = (t.point.position, t.point.normal);
        let index = t.point.index as u64;
        let (done, count) = (self.done.of(seen), self.step.of(seen));
        if done == 0 && count > 0 {
            let mut probes = Rng::probes(s.seed, mesh, index);
            t.inside = !tracer.valid(position, normal, s.backface_tolerance, &mut probes);
        }
        if t.inside {
            return (self.show || self.last).then_some(TexelLight::INSIDE);
        }
        tracer.gather(
            position,
            normal,
            (count, s.bounces),
            &mut t.rng,
            &mut t.paths,
        );
        if self.last {
            return Some(tracer.resolve(position, normal, s.samples, &t.paths, &mut t.rng));
        }
        if !self.show {
            return None;
        }
        let shown = t.shown.get_or_insert_with(|| {
            let mut display = Rng::preview(s.seed, mesh, index);
            tracer.shown(position, normal, s.samples, &mut display)
        });
        Some(t.paths.light(done + count, shown))
    }
}
