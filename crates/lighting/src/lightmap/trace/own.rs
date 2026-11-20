//! A receiving texel's own direct light ([`Tracer::direct`]): every `Baked` lamp
//! in range, an Area light's shadow probed before its full set (#2355).

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::lights::area::{block_of, block_span, cell_point, strata, PROBE};
use super::super::rng::Rng;
use super::direct::{casts, Lit};
use super::{Tracer, T_MIN};
use rusty_core::color::luminance;

impl Tracer<'_> {
    /// The direct irradiance of the `Baked` lamps at a receiving texel's `point`
    /// facing `normal`, shadowed, each weighted by the scene's diffuse model
    /// (`DiffuseModel::response`, #1698), and the luminance-weighted sum of the
    /// directions it arrived from. Every such lamp in range counts (this part is
    /// never filtered, so no lamp is sampled away): an Area light that
    /// [reaches](super::super::lights::area::AreaShape::reaches) `point` over
    /// `area_samples` or more stratified points drawn from `rng` (#1674), shadowed
    /// as [`Self::area`] decides, the rest with one shadow ray each.
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

    /// Area light `light`'s direct light at `point` facing `normal`, over the
    /// `m × m` jittered, stratified points of `m = strata(samples)` drawn from
    /// `rng` (#2355). Where [`probes`] says the probe pays, it first casts a
    /// shadow ray to each of the panel's four corners (inset by [`INSET`]) and to
    /// one point picked in each of the set's `PROBE × PROBE` blocks. Mixed, or
    /// nothing cast: every other point of the set casts its ray. Unanimous, the
    /// verdict decides the rest's light (all of it unshadowed, or none), and each
    /// other point casts its ray anyway with probability `p` ([`chance`], about
    /// [`LIT_CHECKS`] or [`HIDDEN_CHECKS`] rays a panel); a ray that finds the
    /// verdict wrong corrects it by that point's light over `p`.
    ///
    /// The estimate is unbiased whatever the probe says: a probe that misses a
    /// thin occluder or a slit costs noise on that texel, not a missing shadow
    /// or a missing light. Where it was right the correction is zero, so a fully
    /// lit or fully hidden texel is as quiet as the exhaustive estimate. The
    /// probe's points are points of the set, so a penumbra casts only the four
    /// corners more than it. `shadow_probe_tests` and `slit_probe_tests` measure
    /// it against the exhaustive estimate.
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
            return self.rest(&set, Verdict::Cast, |_| false, rng);
        }
        let mut tally = Tally::default();
        for u in CORNERS {
            self.probe_ray(&set, u, &mut tally, false);
        }
        let picks = self.probe_points(&set, &mut tally, rng);
        let verdict = match (tally.passed, tally.blocked) {
            (0, 0) => Verdict::Cast,
            (_, 0) => Verdict::Lit(chance(m, LIT_CHECKS)),
            (0, _) => Verdict::Hidden(chance(m, HIDDEN_CHECKS)),
            _ => Verdict::Cast,
        };
        let probed = |k: u32| picks[block_of(k, m)] == k;
        let rest = self.rest(&set, verdict, probed, rng);
        (tally.lit.0 + rest.0, tally.lit.1 + rest.1)
    }

    /// One point picked in each probe block of `set`, each cast through
    /// [`Self::probe_ray`] into `tally`: the cells picked.
    fn probe_points(&self, set: &Set, tally: &mut Tally, rng: &mut Rng) -> [u32; BLOCKS] {
        let m = set.m;
        let mut cells = [u32::MAX; BLOCKS];
        for (b, cell) in cells.iter_mut().enumerate() {
            let ((x0, x1), (y0, y1)) = (
                block_span(b as u32 % PROBE, m),
                block_span(b as u32 / PROBE, m),
            );
            let n = (x1 - x0) * (y1 - y0);
            let q = ((rng.next_f32() * n as f32) as u32).min(n - 1);
            *cell = (y0 + q / (x1 - x0)) * m + x0 + q % (x1 - x0);
            let u = cell_point(*cell, m, jitter(rng));
            self.probe_ray(set, u, tally, true);
        }
        cells
    }

    /// The shadow ray to `set`'s panel point `u`, if it casts one, counted into
    /// `tally`, and its light added when it passes and `counts` (a point of the
    /// set, not a corner).
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

    /// The light of `set`'s points the probe did not cast (`probed` says which it
    /// did), in row order, as `verdict` has it.
    fn rest(
        &self,
        set: &Set,
        verdict: Verdict,
        probed: impl Fn(u32) -> bool,
        rng: &mut Rng,
    ) -> Lit {
        let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
        for k in (0..set.m * set.m).filter(|&k| !probed(k)) {
            let u = cell_point(k, set.m, jitter(rng));
            let (lit, d) = match verdict {
                Verdict::Cast => self.own_lamp(set.light, set.point, set.normal, u, set.weight),
                // A checked point whose ray finds the verdict wrong moves the
                // estimate by its light over `p`, which keeps it unbiased.
                Verdict::Lit(p) => {
                    let (lit, d) = self.unshadowed(set, u);
                    match rng.next_f32() < p && lit != Vec3::ZERO && self.blocked(set, u) {
                        true => (lit - lit / p, d - d / p),
                        false => (lit, d),
                    }
                }
                Verdict::Hidden(p) => {
                    let (lit, d) = self.unshadowed(set, u);
                    match rng.next_f32() < p && lit != Vec3::ZERO && !self.blocked(set, u) {
                        true => (lit / p, d / p),
                        false => (Vec3::ZERO, Vec3::ZERO),
                    }
                }
            };
            (e, dir) = (e + lit, dir + d);
        }
        (e, dir)
    }

    /// `set`'s light through panel point `u` and its luminance-weighted
    /// direction, no shadow ray cast: zero where none would be.
    fn unshadowed(&self, set: &Set, u: Vec2) -> Lit {
        let arrived = self.received(set.light, set.point, set.normal, u, set.weight);
        match arrived {
            Some((to, _, lit)) if casts(&arrived) => (lit, to * luminance(lit)),
            _ => (Vec3::ZERO, Vec3::ZERO),
        }
    }

    /// Whether the shadow ray from `set`'s point to panel point `u` is blocked,
    /// counted apart (`Rays::count_direct`); only asked where light arrives.
    fn blocked(&self, set: &Set, u: Vec2) -> bool {
        let arrived = self.received(set.light, set.point, set.normal, u, set.weight);
        let Some((to, d, _)) = arrived else {
            return false;
        };
        self.rays.count_direct();
        self.rays.occluded(set.point, to, T_MIN, d)
    }

    /// [`Self::lamp`] for a receiving texel's own direct light: its shadow ray, if
    /// it casts one, counted apart (`Rays::count_direct`) for the cost bench.
    fn own_lamp(&self, light: &BakeLight, point: Vec3, normal: Vec3, u: Vec2, weight: f32) -> Lit {
        let arrived = self.received(light, point, normal, u, weight);
        if casts(&arrived) {
            self.rays.count_direct();
        }
        self.shadowed(point, arrived)
    }
}

/// The probe's blocks, one picked point in each.
const BLOCKS: usize = (PROBE * PROBE) as usize;

/// How far inside the panel's edges, as a fraction of each side, the probe's
/// corner rays aim. A corner exactly on the edge sits where a coffer's or a
/// recess's wall meets the panel's plane, so rounding blocks its ray and a
/// fully seen panel would take the full set. A straight occluder edge that
/// crosses the panel still hides an inset corner unless all it covers lies in
/// that border, at most this fraction of the panel.
const INSET: f32 = 1.0 / 64.0;

/// The four corner points the probe casts at, [`INSET`] inside the panel.
const CORNERS: [Vec2; 4] = [
    Vec2::new(INSET, INSET),
    Vec2::new(1.0 - INSET, INSET),
    Vec2::new(INSET, 1.0 - INSET),
    Vec2::new(1.0 - INSET, 1.0 - INSET),
];

/// The probe's rays: the corners and one point per block.
const PROBE_RAYS: u32 = 4 + BLOCKS as u32;

/// The checking rays a probe that found a panel fully seen casts among the rest
/// of its set, on average ([`chance`]): 24 rays in all, few because most panels
/// over a floor are fully seen, so it keeps most of the probe's saving. Their count, not the set's, sets the noise a wrongly
/// seen panel (a thin occluder all 20 probe rays missed) leaves on the texel:
/// each check stands for `1 / p` points, whatever `directSamples` is.
const LIT_CHECKS: u32 = 4;

/// The checking rays a probe that found a panel fully hidden casts among the
/// rest of its set, on average: 52 rays in all, more than [`LIT_CHECKS`] because
/// a hidden panel is rarer over a floor, so a texel lit only through a slit the
/// probe missed is less noisy than one under a rod it missed.
const HIDDEN_CHECKS: u32 = 32;

/// The probability each point a unanimous probe left is cast anyway, for an
/// `m × m` set: `checks` rays among its `m² − PROBE²` points on average.
fn chance(m: u32, checks: u32) -> f32 {
    (checks as f32 / (m * m - PROBE * PROBE) as f32).min(1.0)
}

/// Whether an `m × m` set is probed: when a unanimous probe and its checks cost
/// fewer rays than the set, whichever way it finds the panel.
fn probes(m: u32) -> bool {
    m >= PROBE && m * m > PROBE_RAYS + LIT_CHECKS.max(HIDDEN_CHECKS)
}

/// What a probe decided about the rest of an Area light's set: cast every
/// point's ray (no probe, mixed, or nothing cast), or take it lit or hidden and
/// check each point with the probability carried.
#[derive(Clone, Copy)]
enum Verdict {
    Cast,
    Lit(f32),
    Hidden(f32),
}

/// One Area light's stratified set at a receiving point: `m × m` points, each
/// carrying `weight` of the panel's light.
struct Set<'a> {
    light: &'a BakeLight,
    point: Vec3,
    normal: Vec3,
    m: u32,
    weight: f32,
}

/// What a probe's shadow rays found: how many passed and were blocked, and the
/// light of the set's points among those that passed.
#[derive(Default)]
struct Tally {
    passed: u32,
    blocked: u32,
    lit: Lit,
}

/// A cell's jitter: two values from the texel's stream.
fn jitter(rng: &mut Rng) -> Vec2 {
    Vec2::new(rng.next_f32(), rng.next_f32())
}
