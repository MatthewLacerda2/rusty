//! A receiving texel's own direct light ([`Tracer::direct`]): every `Baked` lamp
//! in range, an Area light's shadow probed before its full set (#2355).

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::lights::area::{block_of, block_span, cell_point, strata, PROBE};
use super::super::rng::Rng;
use super::check::{below, check_cells, HIDDEN_CHECKS, LIT_CHECKS};
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
    /// verdict decides the rest's light (all of it unshadowed, or none), and
    /// [`LIT_CHECKS`] or [`HIDDEN_CHECKS`] other points, stratified over the
    /// blocks ([`check_cells`]), cast their rays anyway; the first that finds the
    /// verdict wrong drops it and the rest of the set is cast in full
    /// ([`Self::rest`]).
    ///
    /// So a texel's estimate is either the verdict, with every ray it cast
    /// agreeing, or, contradicted, the full set's with the 16 block points the
    /// probe cast kept: nothing is weighted, so no speckle, but those 16 all
    /// agreed with the dropped verdict, so such a texel reads a shade lighter
    /// under a thin occluder (darker beside a thin opening) than the exhaustive
    /// estimate, by about their share of the set. A panel partly under the
    /// receiver's horizon is judged and confirmed by the points whose light
    /// reaches it only ([`casts`]). What can still be lost is an occluder or a slit thin enough to dodge the
    /// corners, the 16 block points and the checks; the texel that misses it is
    /// off by at most its share of that one panel's light. The probe's points
    /// are points of the set, so a penumbra casts only the four corners more
    /// than it. `shadow_probe_tests` and `slit_probe_tests` measure it against
    /// the exhaustive estimate.
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
            let q = below((x1 - x0) * (y1 - y0), rng);
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
    /// did), in row order, as `verdict` has it. Unanimous, the points `checked`
    /// says cast their rays anyway; the first that contradicts the verdict drops
    /// it and the rest of the set is cast in full from fresh points, as a
    /// penumbra's is. Nothing is weighted, so a wrong verdict a check finds
    /// costs rays, never a speckle.
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
                    // A hidden verdict's light is zero, so only a checked cell
                    // needs to know whether light arrives through it.
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
pub(super) const BLOCKS: usize = (PROBE * PROBE) as usize;

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

/// Whether an `m × m` set is probed: when a unanimous probe and its checks cost
/// fewer rays than the set, whichever way it finds the panel.
fn probes(m: u32) -> bool {
    m >= PROBE && m * m > PROBE_RAYS + LIT_CHECKS.max(HIDDEN_CHECKS)
}

/// What a probe decided about the rest of an Area light's set: cast every
/// point's ray (no probe, mixed, or nothing cast), or take it lit or hidden,
/// checking a few of its points ([`check_cells`]).
#[derive(Clone, Copy)]
enum Verdict {
    Cast,
    Lit,
    Hidden,
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
