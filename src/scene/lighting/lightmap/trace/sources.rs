//! What a path vertex can aim a shadow ray at (#1883): the scene's lamps, found
//! through a [`LightGrid`] so only those in range are weighed, and its emissive
//! surfaces ([`Emitter`]) near enough to be worth aiming at. [`Tracer::next_event`](super::Tracer) picks one per
//! vertex, with probability proportional to its weight here.

use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, BakeScene, LightShape};
use super::super::rays::Tri;
use super::super::rng::Rng;
use super::arrival;
use super::emitter::{emitters, Emitter, NONE};
use super::grid::LightGrid;
use crate::components::falloff_window;
use crate::core::color::luminance;
use crate::scene::lighting::DiffuseModel;

/// One thing a vertex may sample: a lamp by its index in `BakeScene::lights`, or
/// an emitter by its index in [`Sources::emitters`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Source {
    Lamp(usize),
    Emitter(usize),
}

/// The scene's light sources, ready for picking.
#[derive(Debug, Default)]
pub(super) struct Sources {
    pub grid: LightGrid,
    pub emitters: Vec<Emitter>,
    /// The emitters by [reach](Emitter::reach).
    pub near: LightGrid,
    /// Per mesh and material run, the emitter it is (`emitter::NONE` if none).
    by_run: Vec<Vec<u32>>,
}

impl Sources {
    pub(super) fn build(scene: &BakeScene) -> Self {
        let (emitters, by_run) = emitters(scene);
        Self {
            grid: LightGrid::build(&scene.lights),
            near: LightGrid::of(emitters.iter().map(|e| Some(e.reach())).collect()),
            emitters,
            by_run,
        }
    }

    /// The emitter triangle `tri` belongs to, if it emits.
    pub(super) fn emitter_at(&self, scene: &BakeScene, tri: &Tri) -> Option<usize> {
        let runs = &scene.meshes[tri.mesh as usize].runs;
        let r = runs
            .partition_point(|r| r.end <= tri.first)
            .min(runs.len().saturating_sub(1));
        let k = *self.by_run.get(tri.mesh as usize)?.get(r)?;
        (k != NONE).then_some(k as usize)
    }
}

/// The sources one vertex weighed, each with its weight, and their sum: what
/// [`Tracer::next_event`](super::Tracer) picked from, kept so a bounce ray that
/// then lands on an emitter can ask how likely that emitter was.
#[derive(Debug, Default)]
pub(super) struct Picks {
    pub list: Vec<(Source, f32)>,
    pub total: f32,
}

impl Picks {
    /// Sum the weights and pick one source by them, with the probability it was
    /// picked with; `None` when nothing was weighed (or the sum is not finite).
    pub(super) fn pick(&mut self, rng: &mut Rng) -> Option<(Source, f32)> {
        self.total = self.list.iter().map(|(_, w)| w).sum();
        if !self.total.is_finite() {
            self.total = 0.0;
        }
        let &(last, last_w) = self.list.last().filter(|_| self.total > 0.0)?;
        let mut u = rng.next_f32() * self.total;
        let (source, w) = (self.list.iter().copied())
            .find(|&(_, w)| {
                u < w || {
                    u -= w;
                    false
                }
            })
            .unwrap_or((last, last_w));
        Some((source, w / self.total))
    }

    /// The probability `source` was picked with (0 if it was not weighed).
    pub(super) fn chance(&self, source: Source) -> f32 {
        let w = self
            .list
            .iter()
            .find(|(s, _)| *s == source)
            .map_or(0.0, |(_, w)| *w);
        match self.total > 0.0 {
            true => w / self.total,
            false => 0.0,
        }
    }
}

/// How much lamp `light` is worth aiming at from `point` facing `normal` under
/// `diffuse`: the luminance it delivers there unshadowed, exactly, for a point,
/// spot or directional lamp; for an Area panel a bound (its power over the squared
/// distance to its nearest point, held to `π · L`), zero out of reach or wholly
/// behind the receiver. Positive wherever the lamp can light `point`, so picking by
/// it stays unbiased.
pub(super) fn lamp_weight(
    light: &BakeLight,
    point: Vec3,
    normal: Vec3,
    diffuse: DiffuseModel,
) -> f32 {
    match light.shape {
        LightShape::Area(area) => {
            if !area.reaches(point) {
                return 0.0;
            }
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .map(|(x, y)| area.point(Vec2::new(x, y)));
            let behind = corners.iter().all(|c| (*c - point).dot(normal) <= 0.0);
            if behind && diffuse == DiffuseModel::Lambert {
                return 0.0;
            }
            let window = falloff_window(point.distance(area.center), area.range);
            let a = area.area();
            let d2 = nearest_on_rect(&area, point).distance_squared(point);
            luminance(light.radiance) * a * window / d2.max(a / std::f32::consts::PI)
        }
        _ => arrival(light, point, Vec2::ZERO).map_or(0.0, |(to_light, _, received)| {
            luminance(received) * diffuse.response(normal.dot(to_light))
        }),
    }
}

/// The point of `area`'s rectangle nearest `p`.
fn nearest_on_rect(area: &super::super::lights::area::AreaShape, p: Vec3) -> Vec3 {
    let along = |edge: Vec3| {
        let l2 = edge.length_squared();
        match l2 > 0.0 {
            true => ((p - area.center).dot(edge) / l2).clamp(-0.5, 0.5),
            false => 0.0,
        }
    };
    area.center + area.right * along(area.right) + area.up * along(area.up)
}
