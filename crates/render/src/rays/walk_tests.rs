use glam::{Mat4, Quat, Vec3};
use rapier3d::parry::query::{Ray, RayCast};
use rapier3d::parry::shape::TriMesh;

use super::fixtures::{rays, Fixture};
use super::top::face;
use super::walk::{trace_any, trace_closest, Hit};

fn oracle(f: &Fixture) -> (TriMesh, Vec<[Vec3; 3]>) {
    let tris = f.world_tris();
    let points = tris.iter().flat_map(|t| *t).collect();
    let indices = (0..tris.len() as u32).map(|i| [3 * i, 3 * i + 1, 3 * i + 2]);
    (
        TriMesh::new(points, indices.collect()).expect("triangles"),
        tris,
    )
}

fn near_edge(p: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let to = |s: Vec3, e: Vec3| {
        let t = ((p - s).dot(e - s) / (e - s).length_squared()).clamp(0.0, 1.0);
        (s + (e - s) * t).distance(p)
    };
    to(a, b).min(to(b, c)).min(to(c, a)) < 1e-3
}

/// Whether `p` lies on `tri`, within `eps`.
fn on(p: Vec3, [a, b, c]: [Vec3; 3], eps: f32) -> bool {
    let n = (b - a).cross(c - a).normalize();
    let inside = [(a, b), (b, c), (c, a)]
        .iter()
        .all(|&(s, e)| (e - s).cross(p - s).dot(n) >= -eps);
    (p - a).dot(n).abs() < eps && (inside || near_edge(p, [a, b, c]))
}

/// Traces every ray both ways; returns how many disagreed only at an edge.
fn agrees(f: &Fixture) -> usize {
    let packed = f.packed();
    let (mesh, tris) = oracle(f);
    let mut ties = 0;
    for (o, d, reach) in rays(f, 400) {
        let ours: Option<Hit> = trace_closest(&packed, o, d, reach);
        assert_eq!(ours.is_some(), trace_any(&packed, o, d, reach));
        let theirs = mesh.cast_local_ray(&Ray::new(o, d), reach, true);
        let edge = |p: Vec3| tris.iter().any(|&t| on(p, t, 1e-4) && near_edge(p, t));
        match (ours, theirs) {
            (Some(h), Some(t)) => {
                assert!(
                    (h.t - t).abs() <= 1e-3 * t.max(1.0),
                    "{}: t {} vs {t}",
                    f.name,
                    h.t
                );
                let p = o + d * t;
                let mine = f.tri_of(&packed, h);
                assert!(on(p, mine, 1e-3), "{}: {mine:?} does not hold {p}", f.name);
                ties += usize::from(edge(p));
            }
            (None, None) => {}
            (Some(h), None) => {
                assert!(edge(o + d * h.t), "{}: a hit parry misses", f.name);
                ties += 1;
            }
            (None, Some(t)) => {
                assert!(edge(o + d * t), "{}: a miss parry hits at {t}", f.name);
                ties += 1;
            }
        }
    }
    ties
}

#[test]
fn the_walk_hits_what_parry_hits_on_every_fixture() {
    for f in Fixture::all() {
        let ties = agrees(&f);
        assert!(ties <= 4, "{}: {ties} edge ties of 400 rays", f.name);
    }
}

#[test]
fn a_hit_takes_the_face_its_material_draws() {
    let at = |face, world| Fixture::one(face, world);
    let (outside, inside) = ((Vec3::Z * 5.0, -Vec3::Z), (Vec3::ZERO, Vec3::Z));
    let hits =
        |f: &Fixture, (o, d): (Vec3, Vec3)| trace_closest(&f.packed(), o, d, 50.0).map(|h| h.t);
    let mirrored = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    assert_eq!(hits(&at(face::FRONT, Mat4::IDENTITY), outside), Some(4.5));
    assert_eq!(hits(&at(face::FRONT, Mat4::IDENTITY), inside), None);
    assert_eq!(hits(&at(face::BACK, Mat4::IDENTITY), inside), Some(0.5));
    assert_eq!(hits(&at(face::BACK, Mat4::IDENTITY), outside), Some(5.5));
    assert_eq!(hits(&at(face::FRONT, mirrored), outside), Some(4.5));
    assert_eq!(hits(&at(face::SKIP, Mat4::IDENTITY), outside), None);
    let turned = Mat4::from_rotation_translation(Quat::from_rotation_y(0.3), Vec3::X * 0.2);
    assert!(hits(&at(face::BOTH, turned), outside).is_some());
}

#[test]
fn a_ray_stops_at_its_reach() {
    let f = Fixture::one(face::FRONT, Mat4::IDENTITY);
    let p = f.packed();
    assert!(trace_closest(&p, Vec3::Z * 5.0, -Vec3::Z, 4.4).is_none());
    assert!(!trace_any(&p, Vec3::Z * 5.0, -Vec3::Z, 4.4));
}
