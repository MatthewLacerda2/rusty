use glam::Vec3;

use super::super::input::BakeScene;
use super::super::lights::area::EMIT_OFFSET;
use rusty_components::FALLOFF_EPSILON_SQ;
use rusty_core::color::luminance;

#[derive(Clone, Copy, Debug)]
struct EmitTri {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
    normal: Vec3,
    upto: f32,
}

#[derive(Clone, Debug)]
pub(super) struct Emitter {
    radiance: Vec3,
    tris: Vec<EmitTri>,
    area: f32,
    lo: Vec3,
    hi: Vec3,
}

const REACH: f32 = 16.0;
const MAX_REACH: f32 = 16.0;

pub(super) const NONE: u32 = u32::MAX;

pub(super) fn emitters(scene: &BakeScene) -> (Vec<Emitter>, Vec<Vec<u32>>) {
    let mut out = Vec::new();
    let mut by_run = Vec::with_capacity(scene.meshes.len());
    for mesh in &scene.meshes {
        let mut runs = vec![NONE; mesh.runs.len()];
        let mut start = 0;
        for (r, run) in mesh.runs.iter().enumerate() {
            let range = start as usize..(run.end as usize).min(mesh.indices.len());
            start = run.end;
            let seen = run.occludes && mesh.cast_shadows.is_visible();
            if !seen || run.emissive.max_element() <= 0.0 || !run.emissive.is_finite() {
                continue;
            }
            let mut e = Emitter {
                radiance: run.emissive,
                tris: Vec::new(),
                area: 0.0,
                lo: Vec3::INFINITY,
                hi: Vec3::NEG_INFINITY,
            };
            for t in mesh.indices[range].chunks_exact(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
                let cross = (b - a).cross(c - a);
                let twice = cross.length();
                if !twice.is_finite() || twice <= 0.0 {
                    continue;
                }
                e.area += twice * 0.5;
                (e.lo, e.hi) = (e.lo.min(a.min(b).min(c)), e.hi.max(a.max(b).max(c)));
                let (e1, e2, normal, upto) = (b - a, c - a, cross / twice, e.area);
                e.tris.push(EmitTri {
                    a,
                    e1,
                    e2,
                    normal,
                    upto,
                });
            }
            if !e.tris.is_empty() {
                runs[r] = out.len() as u32;
                out.push(e);
            }
        }
        by_run.push(runs);
    }
    (out, by_run)
}

impl Emitter {
    /// A bound that only steers the pick, not the light: never 0 where this emitter
    /// can light `point`, or its light is lost.
    pub(super) fn weight(&self, point: Vec3, normal: Vec3) -> f32 {
        let corners = (0..8).map(|k| {
            let pick = |bit: u32, lo: f32, hi: f32| if k & bit == 0 { lo } else { hi };
            Vec3::new(
                pick(1, self.lo.x, self.hi.x),
                pick(2, self.lo.y, self.hi.y),
                pick(4, self.lo.z, self.hi.z),
            )
        });
        if corners.into_iter().all(|c| (c - point).dot(normal) <= 0.0) {
            return 0.0;
        }
        let d2 = (point.clamp(self.lo, self.hi) - point).length_squared();
        luminance(self.radiance) * self.area / d2.max(self.area / std::f32::consts::PI)
    }

    pub(super) fn reach(&self) -> (Vec3, f32) {
        let centre = (self.lo + self.hi) * 0.5;
        let near = (REACH * self.area.sqrt()).min(MAX_REACH);
        (centre, (self.hi - self.lo).length() * 0.5 + near)
    }

    pub(super) fn radiance(&self) -> Vec3 {
        self.radiance
    }

    pub(super) fn pdf(&self, d: f32, cos_emit: f32) -> f32 {
        match cos_emit > 0.0 {
            true => d * d / (cos_emit * self.area),
            false => 0.0,
        }
    }

    pub(super) fn arrival(
        &self,
        point: Vec3,
        normal: Vec3,
        u: [f32; 3],
    ) -> Option<(Vec3, f32, Vec3, f32)> {
        let at = u[0] * self.area;
        let k = self
            .tris
            .partition_point(|t| t.upto <= at)
            .min(self.tris.len() - 1);
        let t = &self.tris[k];
        let (mut s, mut r) = (u[1], u[2]);
        if s + r > 1.0 {
            (s, r) = (1.0 - s, 1.0 - r);
        }
        let target = t.a + t.e1 * s + t.e2 * r + t.normal * EMIT_OFFSET;
        let offset = target - point;
        let d = offset.length();
        if !d.is_finite() || d <= 0.0 {
            return None;
        }
        let dir = offset / d;
        let (cos_emit, cos_recv) = (-dir.dot(t.normal), dir.dot(normal));
        if cos_emit <= 0.0 || cos_recv <= 0.0 {
            return None;
        }
        let scale = cos_emit * cos_recv * self.area / (d * d).max(FALLOFF_EPSILON_SQ);
        Some((dir, d, self.radiance * scale, self.pdf(d, cos_emit)))
    }
}
