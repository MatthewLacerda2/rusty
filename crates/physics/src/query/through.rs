use glam::Vec3;
use rapier3d::prelude::*;

use super::RayHit;
use crate::world::PhysicsWorld;

/// Faces a mesh walk visits at most: a bound on one query's cost.
const MAX_FACES: usize = 256;

/// Metres past a face the next probe starts, so it cannot strike that face twice.
const SKIN: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayCrossing {
    pub enter: RayHit,
    /// `None` when the ray ends inside.
    pub exit: Option<RayHit>,
    pub thickness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    pub enter: f32,
    pub enter_normal: Vec3,
    pub exit: Option<(f32, Vec3)>,
}

impl Span {
    fn crossing(self, id: u32, ray: &Ray, max_toi: f32) -> RayCrossing {
        let hit = |distance, normal| RayHit {
            id,
            distance,
            point: ray.point_at(distance),
            normal,
        };
        RayCrossing {
            enter: hit(self.enter, self.enter_normal),
            exit: self.exit.map(|(d, n)| hit(d, n)),
            thickness: self.exit.map_or(max_toi, |(d, _)| d) - self.enter,
        }
    }
}

/// `ray.dir` must be unit length.
pub(crate) fn spans(shape: &dyn Shape, pose: &Pose, ray: &Ray, max_toi: f32) -> Vec<Span> {
    if let Some(compound) = shape.as_compound() {
        return compound_spans(compound, pose, ray, max_toi);
    }
    match shape.as_trimesh() {
        Some(mesh) => mesh_spans(mesh, pose, ray, max_toi),
        None => convex_span(shape, pose, ray, max_toi).into_iter().collect(),
    }
}

/// Pieces the ray enters within [`SKIN`] of leaving the last are joined.
fn compound_spans(compound: &Compound, pose: &Pose, ray: &Ray, max_toi: f32) -> Vec<Span> {
    let mut pieces: Vec<Span> = (compound.shapes().iter())
        .filter_map(|(at, part)| convex_span(part.as_ref(), &(*pose * *at), ray, max_toi))
        .collect();
    pieces.sort_by(|a, b| a.enter.total_cmp(&b.enter));
    let mut spans: Vec<Span> = Vec::new();
    for piece in pieces {
        match spans.last_mut() {
            Some(last) if last.exit.is_none_or(|(t, _)| piece.enter <= t + SKIN) => {
                let far = |s: &Span| s.exit.map_or(f32::INFINITY, |(t, _)| t);
                if far(&piece) > far(last) {
                    last.exit = piece.exit;
                }
            }
            _ => spans.push(piece),
        }
    }
    spans
}

/// The exit is a reverse cast from past the far side (no chord beats the bounding box diagonal).
fn convex_span(shape: &dyn Shape, pose: &Pose, ray: &Ray, max_toi: f32) -> Option<Span> {
    let enter = shape.cast_ray_and_get_normal(pose, ray, max_toi, true)?;
    let t_in = enter.time_of_impact;
    let enter_normal = if t_in <= 0.0 { -ray.dir } else { enter.normal };
    let reach = shape.compute_aabb(pose).extents().length() * 1.01 + 0.01;
    let far = max_toi.min(t_in + reach);
    let back = Ray::new(ray.point_at(far), -ray.dir);
    let exit = shape
        .cast_ray_and_get_normal(pose, &back, far - t_in, true)
        .filter(|hit| hit.time_of_impact > 0.0)
        .map(|hit| (far - hit.time_of_impact, hit.normal));
    Some(Span {
        enter: t_in,
        enter_normal,
        exit,
    })
}

/// Faces are paired by winding; one contradicting it (open or inside-out mesh) is skipped.
fn mesh_spans(mesh: &TriMesh, pose: &Pose, ray: &Ray, max_toi: f32) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut open: Option<(f32, Vec3)> = None;
    for (i, (t, outward)) in mesh_faces(mesh, pose, ray, max_toi).into_iter().enumerate() {
        let entering = outward.dot(ray.dir) < 0.0;
        match (entering, open) {
            (true, None) => open = Some((t, outward)),
            (false, Some((enter, enter_normal))) => {
                spans.push(Span {
                    enter,
                    enter_normal,
                    exit: Some((t, outward)),
                });
                open = None;
            }
            (false, None) if i == 0 => spans.push(Span {
                enter: 0.0,
                enter_normal: -ray.dir,
                exit: Some((t, outward)),
            }),
            _ => {}
        }
    }
    if let Some((enter, enter_normal)) = open {
        spans.push(Span {
            enter,
            enter_normal,
            exit: None,
        });
    }
    spans
}

fn mesh_faces(mesh: &TriMesh, pose: &Pose, ray: &Ray, max_toi: f32) -> Vec<(f32, Vec3)> {
    let mut faces = Vec::new();
    let mut t = 0.0;
    while faces.len() < MAX_FACES && t <= max_toi {
        let probe = Ray::new(ray.point_at(t), ray.dir);
        let Some(hit) = mesh.cast_ray_and_get_normal(pose, &probe, max_toi - t, false) else {
            break;
        };
        let at = t + hit.time_of_impact;
        let face = mesh.triangle(hit.subshape).scaled_normal();
        faces.push((at, (pose.rotation * face).normalize_or_zero()));
        t = at + SKIN;
    }
    faces
}

impl PhysicsWorld {
    pub fn raycast_through(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        accept: impl Fn(u32) -> bool,
    ) -> Vec<RayCrossing> {
        let ray = Ray::new(origin, dir.normalize());
        let predicate = self.handle_accepts(&accept);
        let filter = QueryFilter::default().predicate(&predicate);
        let mut crossings: Vec<RayCrossing> = Vec::new();
        for (handle, collider, _) in self.queries(filter).intersect_ray(ray, max_toi, true) {
            let Some(&id) = self.collider_to_id.get(&handle) else {
                continue;
            };
            let shape_spans = spans(collider.shape(), collider.position(), &ray, max_toi);
            crossings.extend(
                shape_spans
                    .into_iter()
                    .map(|s| s.crossing(id, &ray, max_toi)),
            );
        }
        crossings.sort_by(|a, b| {
            (a.enter.distance.total_cmp(&b.enter.distance)).then(a.enter.id.cmp(&b.enter.id))
        });
        crossings
    }
}
