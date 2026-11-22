use glam::{Vec2, Vec3};

use super::super::input::{BakeLight, BakeScene, LightShape};
use super::super::rays::Tri;
use super::super::rng::Rng;
use super::arrival;
use super::emitter::{emitters, Emitter, NONE};
use super::grid::LightGrid;
use rusty_components::falloff_window;
use rusty_core::color::luminance;
use rusty_scene::lighting::DiffuseModel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Source {
    Lamp(usize),
    Emitter(usize),
}

#[derive(Debug, Default)]
pub(super) struct Sources {
    pub grid: LightGrid,
    pub emitters: Vec<Emitter>,
    pub near: LightGrid,
    by_run: Vec<Vec<u32>>,
}

impl Sources {
    pub(super) fn build(scene: &BakeScene, tris: &[Tri]) -> Self {
        let (emitters, by_run) = emitters(scene);
        let clip = tris
            .iter()
            .flat_map(|t| [t.a, t.a + t.e1, t.a + t.e2])
            .fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p), hi.max(p))
            });
        let clip = Some(clip).filter(|(lo, hi)| lo.cmple(*hi).all());
        Self {
            grid: LightGrid::build(&scene.lights, clip),
            near: LightGrid::of(emitters.iter().map(|e| Some(e.reach())).collect(), clip),
            emitters,
            by_run,
        }
    }

    pub(super) fn emitter_at(&self, scene: &BakeScene, tri: &Tri) -> Option<usize> {
        let runs = &scene.meshes[tri.mesh as usize].runs;
        let r = runs
            .partition_point(|r| r.end <= tri.first)
            .min(runs.len().saturating_sub(1));
        let k = *self.by_run.get(tri.mesh as usize)?.get(r)?;
        (k != NONE).then_some(k as usize)
    }
}

#[derive(Debug, Default)]
pub(super) struct Picks {
    pub list: Vec<(Source, f32)>,
    pub total: f32,
}

impl Picks {
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

/// Lamp `light`'s pick weight at `point`: exact unshadowed luminance, or a bound for
/// an Area panel. Never 0 where the lamp can light `point`, or that light is lost.
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
