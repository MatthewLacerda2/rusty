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
    /// `rng` (#2355). Past [`PROBE`] cells a side it first probes the panel: a
    /// shadow ray to each of its four corners and to one point picked in each of
    /// the set's `PROBE × PROBE` blocks. All pass: the rest of the set is summed
    /// with no ray. All blocked: up to two more rounds of a point in each block
    /// (`ROUNDS`), and the panel gives nothing only when those are blocked too.
    /// Otherwise (a penumbra, a slit, or nothing cast) the rest of the set casts a
    /// ray each. The probe's points are points of the set, so a penumbra casts
    /// only the four corners more than the exhaustive estimate.
    ///
    /// What it can still miss, each only on texels whose verdict is unanimous:
    /// an occluder's straight edge across the panel always hides a corner, so
    /// every edge's penumbra is kept; an occluder hiding no corner and slipping
    /// between the probe's points (a rod under the panel) casts no shadow of it
    /// there, and a gap showing no corner that the probe's points all miss (a
    /// slit in a cover) lets no light through. `shadow_probe_tests` and
    /// `slit_probe_tests` measure both.
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
        if !self.probe || m <= PROBE {
            return self.rest(&set, true, |_| false, rng);
        }
        let mut tally = Tally::default();
        for u in [Vec2::ZERO, Vec2::X, Vec2::Y, Vec2::ONE] {
            self.probe_ray(&set, u, &mut tally, false);
        }
        let mut picks = [[u32::MAX; BLOCKS]; ROUNDS];
        for round in 0..ROUNDS {
            if round > 0 && !(tally.passed == 0 && tally.blocked > 0) {
                break;
            }
            picks[round] = self.probe_points(&set, &picks[..round], &mut tally, rng);
        }
        if tally.passed == 0 && tally.blocked > 0 {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let shadowed = tally.blocked > 0 || tally.passed == 0;
        let probed = |k: u32| picks.iter().any(|round| round[block_of(k, m)] == k);
        let rest = self.rest(&set, shadowed, probed, rng);
        (tally.lit.0 + rest.0, tally.lit.1 + rest.1)
    }

    /// One point picked in each probe block of `set`, among the cells no earlier
    /// round in `taken` picked there (a block with none left gets none), each cast
    /// through [`Self::probe_ray`] into `tally`: the cells picked, `u32::MAX`
    /// where none was.
    fn probe_points(
        &self,
        set: &Set,
        taken: &[[u32; BLOCKS]],
        tally: &mut Tally,
        rng: &mut Rng,
    ) -> [u32; BLOCKS] {
        let m = set.m;
        let mut cells = [u32::MAX; BLOCKS];
        for (b, cell) in cells.iter_mut().enumerate() {
            let ((x0, x1), (y0, y1)) = (
                block_span(b as u32 % PROBE, m),
                block_span(b as u32 / PROBE, m),
            );
            let at = |q: u32| (y0 + q / (x1 - x0)) * m + x0 + q % (x1 - x0);
            let free = |q: &u32| taken.iter().all(|round| round[b] != at(*q));
            let n = (x1 - x0) * (y1 - y0);
            let left = (0..n).filter(free).count() as u32;
            if left == 0 {
                continue;
            }
            let nth = ((rng.next_f32() * left as f32) as u32).min(left - 1);
            *cell = (0..n).filter(free).nth(nth as usize).map_or(u32::MAX, at);
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
    /// did), in row order, a shadow ray each when `shadowed`, else unshadowed.
    fn rest(&self, set: &Set, shadowed: bool, probed: impl Fn(u32) -> bool, rng: &mut Rng) -> Lit {
        let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
        for k in (0..set.m * set.m).filter(|&k| !probed(k)) {
            let u = cell_point(k, set.m, jitter(rng));
            let (lit, d) = match shadowed {
                true => self.own_lamp(set.light, set.point, set.normal, u, set.weight),
                false => unshadowed(self.received(set.light, set.point, set.normal, u, set.weight)),
            };
            (e, dir) = (e + lit, dir + d);
        }
        (e, dir)
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

/// `arrived`'s light and its luminance-weighted direction, no shadow ray cast:
/// what [`Tracer::area`] sums where its probe found the panel unshadowed.
fn unshadowed(arrived: Option<(Vec3, f32, Vec3)>) -> Lit {
    match arrived {
        Some((to, _, lit)) if casts(&arrived) => (lit, to * luminance(lit)),
        _ => (Vec3::ZERO, Vec3::ZERO),
    }
}

/// Rounds of one point per block the probe casts while every ray so far is
/// blocked, before the panel gives nothing: the first and two confirming ones
/// (#2355, `slit_probe_tests`).
const ROUNDS: usize = 3;

/// The probe's blocks, one picked point in each.
const BLOCKS: usize = (PROBE * PROBE) as usize;

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
