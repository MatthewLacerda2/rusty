//! src/physics/through.rs — the penetration ray: where a ray enters *and leaves*
//! every solid it crosses (#830), the query wallbangs and over-penetration price
//! their damage on.
//!
//! The world cast only says *which* colliders the ray touches; each exit is then
//! found against that one collider's own shape, never by a second world cast, so
//! nested or overlapping colliders can't be mis-paired. A convex shape is crossed
//! at most once, and its exit is a reverse cast from beyond its far side. A
//! triangle mesh (a whole building) can be crossed many times — wall, room, wall
//! — so its faces are walked along the ray and paired by winding: a face whose
//! outward normal opposes the ray is an entry, one facing along it an exit.

use glam::Vec3;
use rapier3d::prelude::*;

use super::query::RayHit;
use super::world::PhysicsWorld;

/// Past this many faces a mesh walk stops: a bound on one query's cost, far
/// beyond any wall a bullet is meant to cross.
const MAX_FACES: usize = 256;

/// How far past a face the next mesh probe starts, so it can't strike the same
/// face (or its neighbour across a shared edge) twice. Walls thinner than this
/// read as a single face and are skipped.
const SKIN: f32 = 1e-4;

/// One stretch of a solid the ray passes through: where it went in, where it
/// came out, and the material crossed between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayCrossing {
    /// The entry. A ray starting inside the solid enters at distance 0, at the
    /// origin, with the normal facing back along the ray (Unity's convention).
    pub enter: RayHit,
    /// The exit, its normal facing out of the solid (along the ray); `None`
    /// when the ray ends inside it.
    pub exit: Option<RayHit>,
    /// Exit distance (or `max_toi` when there is no exit) minus entry distance.
    pub thickness: f32,
}

/// One crossing of one shape, as distances along the ray and outward normals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Span {
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

/// Every crossing of `shape` (placed at `pose`) by `ray` within `max_toi`,
/// nearest first. `ray.dir` must be unit length.
pub(super) fn spans(shape: &dyn Shape, pose: &Pose, ray: &Ray, max_toi: f32) -> Vec<Span> {
    match shape.as_trimesh() {
        Some(mesh) => mesh_spans(mesh, pose, ray, max_toi),
        None => convex_span(shape, pose, ray, max_toi).into_iter().collect(),
    }
}

/// A convex shape's single crossing. The exit comes from a reverse cast that
/// starts past the shape's far side — no chord through it is longer than its
/// bounding box's diagonal — or at `max_toi` when that comes first, where a hit
/// at distance 0 means the ray ends inside.
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

/// A triangle mesh's crossings: walk its faces along the ray, then pair each
/// entry face with the exit face after it. A leading exit face means the ray
/// started inside; a trailing entry means it ends inside. A face the winding
/// contradicts (an open or inside-out mesh) is skipped rather than guessed at.
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

/// Each mesh face the ray passes within `max_toi`, nearest first, as (distance,
/// the face's world-space outward unit normal from its winding).
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
    /// Every crossing of an accepted collider by the ray within `max_toi`, as
    /// enter/exit pairs, nearest entry first (ties by entity id). A concave
    /// mesh crossed twice yields two crossings with the same id. Same filter
    /// and inactive-entity rules as [`Self::raycast_all`]; triggers count.
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
