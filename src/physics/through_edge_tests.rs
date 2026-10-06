//! src/physics/through_edge_tests.rs — `raycast_through`'s edges (#892): a stray
//! exit face after a crossing, the mesh walk's face cap, and a convex exit along
//! the longest chord of a large and a small box.

use glam::Vec3;
use rapier3d::prelude::*;

use super::through::spans;

/// A mesh's vertex and triangle buffers, before `TriMesh::new`.
type Buffers = (Vec<Vec3>, Vec<[u32; 3]>);

/// Appends a closed, outward-wound box (center, half-extents) to `buf`.
fn push_box(buf: &mut Buffers, center: Vec3, half: Vec3) {
    let (vertices, indices) = buf;
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for side in [axis, -axis] {
            let u = side.any_orthonormal_vector();
            let v = side.cross(u);
            let base = vertices.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                vertices.push(center + (side + u * a + v * b) * half);
            }
            // u × v = side, so (0,1,2) winds counter-clockwise seen from outside.
            indices.push([base, base + 1, base + 2]);
            indices.push([base, base + 2, base + 3]);
        }
    }
}

/// Distances of each span's (enter, exit) along `ray`.
fn crossed(shape: &dyn Shape, ray: &Ray, max_toi: f32) -> Vec<(f32, Option<f32>)> {
    let got = spans(shape, &Pose::IDENTITY, ray, max_toi);
    got.iter().map(|s| (s.enter, s.exit.map(|e| e.0))).collect()
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() < tol
}

/// A ray off the faces' diagonals, so no probe grazes a shared edge.
fn along_z() -> Ray {
    Ray::new(Vec3::new(0.1, 0.2, 0.0), Vec3::Z)
}

#[test]
fn a_stray_exit_face_after_a_crossing_is_skipped() {
    let mut buf = Buffers::default();
    push_box(&mut buf, Vec3::new(0.0, 0.0, 3.0), Vec3::ONE);
    let (mut vertices, mut indices) = buf;
    // A lone triangle at z = 6 facing along the ray: an exit with no entry.
    let base = vertices.len() as u32;
    vertices.extend([
        Vec3::new(-2.0, -2.0, 6.0),
        Vec3::new(2.0, -2.0, 6.0),
        Vec3::new(0.0, 2.0, 6.0),
    ]);
    indices.push([base, base + 1, base + 2]);
    let mesh = TriMesh::new(vertices, indices).unwrap();
    let got = crossed(&mesh, &along_z(), 100.0);
    assert_eq!(got.len(), 1, "only the box is crossed: {got:?}");
    assert!(near(got[0].0, 2.0, 1e-3) && near(got[0].1.unwrap(), 4.0, 1e-3));
}

#[test]
fn the_mesh_walk_stops_at_its_face_cap() {
    let mut buf = Buffers::default();
    for i in 0..130 {
        let center = Vec3::new(0.0, 0.0, 1.0 + i as f32 * 0.5);
        push_box(&mut buf, center, Vec3::new(1.0, 1.0, 0.05));
    }
    let mesh = TriMesh::new(buf.0, buf.1).unwrap();
    let got = crossed(&mesh, &along_z(), 1000.0);
    // 256 faces are 128 whole crossings; the walk never reaches slab 129.
    assert_eq!(got.len(), 128, "{got:?}");
    assert!(got.iter().all(|(_, exit)| exit.is_some()), "{got:?}");
    let last = got[127];
    assert!(near(last.0, 64.45, 1e-3) && near(last.1.unwrap(), 64.55, 1e-3));
}

/// A ray through a cube of half-size `h` from near one corner to near the
/// opposite one, its per-axis offsets breaking the exact diagonal so it enters
/// through the -X face and leaves through +Z. Checks entry, exit and thickness.
fn assert_crosses_corner_to_corner(h: f32, dy: f32, dz: f32, tol: f32) {
    let cube = Cuboid::new(Vec3::splat(h));
    let dir = Vec3::ONE.normalize();
    let ray = Ray::new(Vec3::new(-2.0 * h, -2.0 * h + dy, -2.0 * h + dz), dir);
    let got = spans(&cube, &Pose::IDENTITY, &ray, 1000.0);
    let [span] = got[..] else {
        panic!("one crossing: {got:?}");
    };
    let root3 = 3f32.sqrt();
    let (enter, exit) = (h * root3, (3.0 * h - dz) * root3);
    let (out, normal) = span.exit.expect("the exit lies on the far face");
    assert!(near(span.enter, enter, tol), "{span:?}");
    assert!(near(out, exit, tol), "{span:?} want exit {exit}");
    assert!((normal - Vec3::Z).length() < 1e-3, "{span:?}");
    // The chord is nearly the box's full diagonal, 2h·√3.
    assert!(near(out - span.enter, (2.0 * h - dz) * root3, tol));
}

#[test]
fn a_large_box_is_exited_along_its_longest_chord() {
    // 10 units across: the chord is ~0.6% short of the diagonal.
    assert_crosses_corner_to_corner(5.0, 0.03, 0.06, 1e-3);
}

#[test]
fn a_small_box_is_exited_along_its_longest_chord() {
    // 0.2 units across: the chord falls short of the diagonal by under 0.01.
    assert_crosses_corner_to_corner(0.1, 0.0005, 0.001, 1e-4);
}
