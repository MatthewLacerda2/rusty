use super::super::super::raster::TexelPoint;
use super::super::super::rng::Rng;
use super::super::super::trace::{Paths, Shown, TexelLight, Tracer};
use super::super::pool;
use super::schedule::Step;
use rusty_scene::lighting::settings::BakeSettings;

#[derive(Debug)]
pub(super) struct Texel {
    pub point: TexelPoint,
    rng: Rng,
    paths: Paths,
    inside: bool,
    shown: Option<Shown>,
}

pub(super) type Charts = Vec<Vec<Vec<Texel>>>;

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

pub(super) struct Work<'a> {
    pub tracer: &'a Tracer<'a>,
    pub settings: &'a BakeSettings,
    pub done: Step,
    pub step: Step,
    pub last: bool,
    pub show: bool,
}

impl Work<'_> {
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
            return Some(tracer.resolve(
                position,
                normal,
                (s.samples, s.direct_sample_count()),
                &t.paths,
                &mut t.rng,
            ));
        }
        if !self.show {
            return None;
        }
        let shown = t.shown.get_or_insert_with(|| {
            let mut display = Rng::preview(s.seed, mesh, index);
            tracer.shown(position, normal, s.direct_sample_count(), &mut display)
        });
        Some(t.paths.light(done + count, shown))
    }
}
