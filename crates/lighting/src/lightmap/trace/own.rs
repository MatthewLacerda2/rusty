//! A receiving texel's own direct light ([`Tracer::direct`]): every `Baked` lamp
//! in range, an Area light's shadow probed before its full set (#2355).

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::lights::area::{strata, stratified};
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

    /// Area light `light`'s direct light at `point` facing `normal`, over
    /// `strata(samples)²` stratified points drawn from `rng` (#2355). Past the
    /// tracer's probe grid ([`PROBE`](super::super::lights::area::PROBE)² cells)
    /// it first casts a probe: a shadow ray to each of the panel's four corners,
    /// then a coarse stratified set. Where all the probe's rays pass, the panel is
    /// taken as unshadowed and the full set is summed with no ray; where all are
    /// blocked, it gives nothing; mixed (a penumbra), or a probe with no ray to
    /// cast, the full set casts a ray each. An occluder's straight edge across the
    /// panel always hides a corner, so the corners keep every edge's penumbra;
    /// what the probe can miss is an occluder that hides no corner and slips
    /// between the stratified rays, thinner than their spacing (a quarter of the
    /// panel) as the texel sees it: it casts no shadow of this panel there.
    fn area(
        &self,
        light: &BakeLight,
        point: Vec3,
        normal: Vec3,
        samples: u32,
        rng: &mut Rng,
    ) -> Lit {
        let m = strata(samples);
        let weight = 1.0 / (m * m) as f32;
        let full = |shadowed: bool, rng: &mut Rng| {
            let (mut e, mut dir) = (Vec3::ZERO, Vec3::ZERO);
            for u in stratified(m, || rng.next_f32()) {
                let (lit, d) = match shadowed {
                    true => self.own_lamp(light, point, normal, u, weight),
                    false => unshadowed(self.received(light, point, normal, u, weight)),
                };
                (e, dir) = (e + lit, dir + d);
            }
            (e, dir)
        };
        if self.probe == 0 || m <= self.probe {
            return full(true, rng);
        }
        let (mut passed, mut blocked) = (0u32, 0u32);
        let corners = [Vec2::ZERO, Vec2::X, Vec2::Y, Vec2::ONE];
        for u in corners
            .into_iter()
            .chain(stratified(self.probe, || rng.next_f32()))
        {
            let arrived = self.received(light, point, normal, u, 1.0);
            let Some((to, d, _)) = arrived.filter(|_| casts(&arrived)) else {
                continue;
            };
            self.rays.count_direct();
            match self.rays.occluded(point, to, T_MIN, d) {
                true => blocked += 1,
                false => passed += 1,
            }
        }
        match (passed, blocked) {
            (0, b) if b > 0 => (Vec3::ZERO, Vec3::ZERO),
            (p, 0) if p > 0 => full(false, rng),
            _ => full(true, rng),
        }
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
