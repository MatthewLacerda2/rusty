use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::lights::area::{block_of, block_span, cell_point, strata, PROBE};
use super::super::rng::Rng;
use super::check::{below, check_cells, HIDDEN_CHECKS, LIT_CHECKS};
use super::direct::{casts, Lit};
use super::{Tracer, T_MIN};
use rusty_core::color::luminance;

impl Tracer<'_> {
    pub(super) fn direct(
        &self,
        point: Vec3,
        normal: Vec3,
        area_samples: u32,
        rng: &mut Rng,
    ) -> Lit {
        let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
        for i in self.sources.grid.near(point) {
            let light = &self.scene.lights[i];
            if !light.bakes_direct {
                continue;
            }
            match light.shape {
                LightShape::Area(area) if area.reaches(point) => {
                    let (lit, d) = self.area(light, point, normal, area_samples, rng);
                    (e, dir) = (e + lit, dir + d);
                }
                LightShape::Area(_) => {}
                _ => {
                    let (lit, d) = self.own_lamp(light, point, normal, Vec2::ZERO, 1.0);
                    (e, dir) = (e + lit, dir + d);
                }
            }
        }
        (e, dir)
    }

    fn area(
        &self,
        light: &BakeLight,
        point: Vec3,
        normal: Vec3,
        samples: u32,
        rng: &mut Rng,
    ) -> Lit {
        let m = strata(samples);
        let set = Set {
            light,
            point,
            normal,
            m,
            weight: 1.0 / (m * m) as f32,
        };
        if !self.probe || !probes(m) {
            return self.rest(&set, Verdict::Cast, |_| false, |_| false, rng);
        }
        let mut tally = Tally::default();
        for u in CORNERS {
            self.probe_ray(&set, u, &mut tally, false);
        }
        let picks = self.probe_points(&set, &mut tally, rng);
        let verdict = match (tally.passed, tally.blocked) {
            (0, 0) => Verdict::Cast,
            (_, 0) => Verdict::Lit,
            (0, _) => Verdict::Hidden,
            _ => Verdict::Cast,
        };
        let checks = match verdict {
            Verdict::Cast => [[u32::MAX; 2]; BLOCKS],
            _ => check_cells(m, &picks, matches!(verdict, Verdict::Lit), rng),
        };
        let probed = |k: u32| picks[block_of(k, m)] == k;
        let checked = |k: u32| checks[block_of(k, m)].contains(&k);
        let rest = self.rest(&set, verdict, probed, checked, rng);
        (tally.lit.0 + rest.0, tally.lit.1 + rest.1)
    }

    fn probe_points(&self, set: &Set, tally: &mut Tally, rng: &mut Rng) -> [u32; BLOCKS] {
        let m = set.m;
        let mut cells = [u32::MAX; BLOCKS];
        for (b, cell) in cells.iter_mut().enumerate() {
            let ((x0, x1), (y0, y1)) = (
                block_span(b as u32 % PROBE, m),
                block_span(b as u32 / PROBE, m),
            );
            let q = below((x1 - x0) * (y1 - y0), rng);
            *cell = (y0 + q / (x1 - x0)) * m + x0 + q % (x1 - x0);
            let u = cell_point(*cell, m, jitter(rng));
            self.probe_ray(set, u, tally, true);
        }
        cells
    }

    fn probe_ray(&self, set: &Set, u: Vec2, tally: &mut Tally, counts: bool) {
        let arrived = self.received(set.light, set.point, set.normal, u, set.weight);
        let Some((to, d, lit)) = arrived.filter(|_| casts(&arrived)) else {
            return;
        };
        self.rays.count_direct();
        if self.rays.occluded(set.point, to, T_MIN, d) {
            tally.blocked += 1;
            return;
        }
        tally.passed += 1;
        if counts {
            tally.lit = (tally.lit.0 + lit, tally.lit.1 + to * luminance(lit));
        }
    }

    fn rest(
        &self,
        set: &Set,
        verdict: Verdict,
        probed: impl Fn(u32) -> bool + Copy,
        checked: impl Fn(u32) -> bool + Copy,
        rng: &mut Rng,
    ) -> Lit {
        let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
        for k in (0..set.m * set.m).filter(|&k| !probed(k)) {
            let u = cell_point(k, set.m, jitter(rng));
            let (lit, d) = match verdict {
                Verdict::Cast => self.own_lamp(set.light, set.point, set.normal, u, set.weight),
                Verdict::Lit | Verdict::Hidden => {
                    let seen = matches!(verdict, Verdict::Lit);
                    let (lit, d) = if seen || checked(k) {
                        self.unshadowed(set, u)
                    } else {
                        (Vec3::ZERO, Vec3::ZERO)
                    };
                    if checked(k) && lit != Vec3::ZERO && self.blocked(set, u) == seen {
                        return self.rest(set, Verdict::Cast, probed, checked, rng);
                    }
                    if seen {
                        (lit, d)
                    } else {
                        (Vec3::ZERO, Vec3::ZERO)
                    }
                }
            };
            (e, dir) = (e + lit, dir + d);
        }
        (e, dir)
    }

    fn unshadowed(&self, set: &Set, u: Vec2) -> Lit {
        let arrived = self.received(set.light, set.point, set.normal, u, set.weight);
        match arrived {
            Some((to, _, lit)) if casts(&arrived) => (lit, to * luminance(lit)),
            _ => (Vec3::ZERO, Vec3::ZERO),
        }
    }

    fn blocked(&self, set: &Set, u: Vec2) -> bool {
        let arrived = self.received(set.light, set.point, set.normal, u, set.weight);
        let Some((to, d, _)) = arrived else {
            return false;
        };
        self.rays.count_direct();
        self.rays.occluded(set.point, to, T_MIN, d)
    }

    fn own_lamp(&self, light: &BakeLight, point: Vec3, normal: Vec3, u: Vec2, weight: f32) -> Lit {
        let arrived = self.received(light, point, normal, u, weight);
        if casts(&arrived) {
            self.rays.count_direct();
        }
        self.shadowed(point, arrived)
    }
}

pub(super) const BLOCKS: usize = (PROBE * PROBE) as usize;

const INSET: f32 = 1.0 / 64.0;

const CORNERS: [Vec2; 4] = [
    Vec2::new(INSET, INSET),
    Vec2::new(1.0 - INSET, INSET),
    Vec2::new(INSET, 1.0 - INSET),
    Vec2::new(1.0 - INSET, 1.0 - INSET),
];

const PROBE_RAYS: u32 = 4 + BLOCKS as u32;

fn probes(m: u32) -> bool {
    m >= PROBE && m * m > PROBE_RAYS + LIT_CHECKS.max(HIDDEN_CHECKS)
}

#[derive(Clone, Copy)]
enum Verdict {
    Cast,
    Lit,
    Hidden,
}

struct Set<'a> {
    light: &'a BakeLight,
    point: Vec3,
    normal: Vec3,
    m: u32,
    weight: f32,
}

#[derive(Default)]
struct Tally {
    passed: u32,
    blocked: u32,
    lit: Lit,
}

fn jitter(rng: &mut Rng) -> Vec2 {
    Vec2::new(rng.next_f32(), rng.next_f32())
}
