use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, LightShape};
use super::super::rng::Rng;
use super::sources::{lamp_weight, Picks, Source};
use super::{Tracer, T_MIN};
use rusty_components::{falloff, spot_cone};
use rusty_core::color::luminance;

pub(super) type Lit = (Vec3, Vec3);

impl Tracer<'_> {
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
                let (lit, dir) = self.unhidden(point, arrived.map(|(to, d, lit, _)| (to, d, lit)));
                (lit * share, dir * share)
            }
        };
        (lit / chance, dir / chance)
    }

    pub(super) fn lamp(
        &self,
        light: &BakeLight,
        point: Vec3,
        normal: Vec3,
        u: Vec2,
        weight: f32,
    ) -> Lit {
        self.shadowed(point, self.received(light, point, normal, u, weight))
    }

    pub(super) fn received(
        &self,
        light: &BakeLight,
        point: Vec3,
        normal: Vec3,
        u: Vec2,
        weight: f32,
    ) -> Option<(Vec3, f32, Vec3)> {
        arrival(light, point, u).map(|(to_light, d, received)| {
            let response = self.scene.diffuse.response(normal.dot(to_light));
            (to_light, d, received * response * weight)
        })
    }

    pub(super) fn shadowed(&self, point: Vec3, arrived: Option<(Vec3, f32, Vec3)>) -> Lit {
        self.visible(point, arrived, |i| self.rays.tris[i].blocks)
    }

    pub(super) fn unhidden(&self, point: Vec3, arrived: Option<(Vec3, f32, Vec3)>) -> Lit {
        self.visible(point, arrived, |i| self.rays.tris[i].seen)
    }

    fn visible(
        &self,
        point: Vec3,
        arrived: Option<(Vec3, f32, Vec3)>,
        stops: impl Fn(usize) -> bool,
    ) -> Lit {
        match arrived {
            Some((to, d, lit))
                if casts(&arrived) && !self.rays.occluded_by(point, to, (T_MIN, d), stops) =>
            {
                (lit, to * luminance(lit))
            }
            _ => (Vec3::ZERO, Vec3::ZERO),
        }
    }
}

pub(super) fn casts(arrived: &Option<(Vec3, f32, Vec3)>) -> bool {
    arrived.is_some_and(|(_, _, lit)| lit.max_element() > 0.0)
}

pub(super) fn balance(this: f32, other: f32) -> f32 {
    match this + other > 0.0 {
        true => this / (this + other),
        false => 0.0,
    }
}

pub(in crate::lightmap) fn arrival(
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

fn toward(position: Vec3, point: Vec3, range: f32) -> Option<(Vec3, f32)> {
    let offset = position - point;
    let d = offset.length();
    (d <= range && d > 0.0).then(|| (offset / d, d))
}
