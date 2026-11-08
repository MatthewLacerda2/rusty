//! Direct light at a point: every `Baked` lamp in range for a receiving texel's
//! own direct light ([`Tracer::direct`]), and one importance-picked source per
//! path vertex for the bounce ([`Tracer::next_event`], #1883).

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::lights::area::{strata, stratified};
use super::super::rng::Rng;
use super::sources::{lamp_weight, Picks, Source};
use super::{Tracer, T_MIN};
use crate::components::{falloff, spot_cone};
use crate::core::color::luminance;

/// Where a vertex's light came from, and the luminance-weighted direction: the
/// shape `direct` and `next_event` both return.
pub(super) type Lit = (Vec3, Vec3);

impl Tracer<'_> {
    /// The direct irradiance of the `Baked` lamps at a receiving texel's `point`
    /// facing `normal`, shadowed, each weighted by the scene's diffuse model
    /// (`DiffuseModel::response`, #1698), and the luminance-weighted sum of the
    /// directions it arrived from. Every such lamp in range counts (this part is
    /// never filtered, so it is not sampled): an Area light that
    /// [reaches](super::super::lights::area::AreaShape::reaches) `point` over
    /// `area_samples` or more stratified points drawn from `rng` (#1674), the
    /// rest with one shadow ray each.
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
                    let m = strata(area_samples);
                    let weight = 1.0 / (m * m) as f32;
                    for u in stratified(m, || rng.next_f32()) {
                        let (lit, d) = self.lamp(light, point, normal, u, weight);
                        (e, dir) = (e + lit, dir + d);
                    }
                }
                LightShape::Area(_) => {}
                _ => {
                    let (lit, d) = self.lamp(light, point, normal, Vec2::ZERO, 1.0);
                    (e, dir) = (e + lit, dir + d);
                }
            }
        }
        (e, dir)
    }

    /// One sample of the light reaching a path vertex at `point` facing `normal`
    /// (next-event estimation, #1883): one source picked from the lamps in range
    /// (when `lamps`) and the emitters near it (when `emitters`), with probability
    /// `p` proportional to its weight (`sources::lamp_weight`, `Emitter::weight`),
    /// one point on it, and its light over `p`. Unbiased, and its cost grows with
    /// the sources in range, not the level's. An emitter's light is weighed
    /// against the path's next bounce ray, which may land on the same point, by
    /// the balance heuristic (`Tracer::path` takes the other share), so emitters
    /// are only asked for where that ray follows. `picks` keeps the weights.
    pub(super) fn next_event(
        &self,
        point: Vec3,
        normal: Vec3,
        (lamps, emitters): (bool, bool),
        rng: &mut Rng,
        picks: &mut Picks,
    ) -> Lit {
        picks.list.clear();
        let diffuse = self.scene.diffuse;
        if lamps {
            for i in self.sources.grid.near(point) {
                let w = lamp_weight(&self.scene.lights[i], point, normal, diffuse);
                if w > 0.0 {
                    picks.list.push((Source::Lamp(i), w));
                }
            }
        }
        if emitters {
            for k in self.sources.near.near(point) {
                let w = self.sources.emitters[k].weight(point, normal);
                if w > 0.0 {
                    picks.list.push((Source::Emitter(k), w));
                }
            }
        }
        let Some((source, chance)) = picks.pick(rng) else {
            return (Vec3::ZERO, Vec3::ZERO);
        };
        let (lit, dir) = match source {
            Source::Lamp(i) => {
                let light = &self.scene.lights[i];
                let u = match light.shape {
                    LightShape::Area(_) => Vec2::new(rng.next_f32(), rng.next_f32()),
                    _ => Vec2::ZERO,
                };
                self.lamp(light, point, normal, u, 1.0)
            }
            Source::Emitter(k) => {
                let u = [rng.next_f32(), rng.next_f32(), rng.next_f32()];
                let arrived = self.sources.emitters[k].arrival(point, normal, u);
                let share = arrived.map_or(1.0, |(to, _, _, pdf)| {
                    balance(chance * pdf, normal.dot(to) / PI)
                });
                let (lit, dir) = self.shadowed(point, arrived.map(|(to, d, lit, _)| (to, d, lit)));
                (lit * share, dir * share)
            }
        };
        (lit / chance, dir / chance)
    }

    /// Lamp `light`'s light at `point` facing `normal` through its point at `u`,
    /// shadowed and times `weight`, with its luminance-weighted direction.
    pub(super) fn lamp(
        &self,
        light: &BakeLight,
        point: Vec3,
        normal: Vec3,
        u: Vec2,
        weight: f32,
    ) -> Lit {
        let arrived = arrival(light, point, u).map(|(to_light, d, received)| {
            let response = self.scene.diffuse.response(normal.dot(to_light));
            (to_light, d, received * response * weight)
        });
        self.shadowed(point, arrived)
    }

    /// `arrived`'s light (direction, distance, irradiance) if it is positive and
    /// no blocker stands between `point` and it.
    pub(super) fn shadowed(&self, point: Vec3, arrived: Option<(Vec3, f32, Vec3)>) -> Lit {
        match arrived {
            Some((to_light, d, lit))
                if lit.max_element() > 0.0 && !self.rays.occluded(point, to_light, T_MIN, d) =>
            {
                (lit, to_light * luminance(lit))
            }
            _ => (Vec3::ZERO, Vec3::ZERO),
        }
    }
}

/// The balance heuristic's share for the strategy whose density is `this`, beside
/// one of density `other` (Veach): the two shares of one path always sum to 1.
pub(super) fn balance(this: f32, other: f32) -> f32 {
    match this + other > 0.0 {
        true => this / (this + other),
        false => 0.0,
    }
}

/// Unit direction to `light`, distance to it, and the radiance arriving at `point`
/// (before the cosine), with the forward shader's falloff and cone (#1699):
/// [`falloff`] inside `range`, and for a spot [`spot_cone`]. An Area light is
/// asked about its point at `u` in `[0, 1]²` (`area`'s model); the rest ignore
/// `u`. `None` out of reach.
pub(in crate::scene::lighting::lightmap) fn arrival(
    light: &BakeLight,
    point: Vec3,
    u: Vec2,
) -> Option<(Vec3, f32, Vec3)> {
    match light.shape {
        LightShape::Area(area) => {
            let (dir, d, scale) = area.arrival(point, u)?;
            Some((dir, d, light.radiance * scale))
        }
        LightShape::Directional { direction } => Some((-direction, f32::MAX, light.radiance)),
        LightShape::Point { position, range } => {
            let (dir, d) = toward(position, point, range)?;
            Some((dir, d, light.radiance * falloff(d, range)))
        }
        LightShape::Spot {
            position,
            direction,
            range,
            cos_inner,
            cos_outer,
        } => {
            let (dir, d) = toward(position, point, range)?;
            let theta = dir.dot(-direction);
            if theta <= cos_outer {
                return None;
            }
            let cone = spot_cone(theta, cos_inner, cos_outer);
            Some((dir, d, light.radiance * cone * falloff(d, range)))
        }
    }
}

/// Unit direction and distance from `point` to `position`, if within `range`.
fn toward(position: Vec3, point: Vec3, range: f32) -> Option<(Vec3, f32)> {
    let offset = position - point;
    let d = offset.length();
    (d <= range && d > 0.0).then(|| (offset / d, d))
}
