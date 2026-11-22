use glam::Vec3;

use super::fixture;
use super::hull::beyond_faces;
use super::mesh::{build_mesh_shape, MeshFit};

fn off_centre_cylinder() -> Vec<[f32; 3]> {
    let (vertices, _) = fixture::off_centre_cylinder();
    vertices.iter().map(|v| v.position).collect()
}

fn worst_outside(points: &[[f32; 3]], scale: Vec3) -> f32 {
    let collider = build_mesh_shape(MeshFit::Hull, scale, (points, &[])).expect("a hull");
    let hull = collider
        .shape()
        .as_convex_polyhedron()
        .expect("a convex polyhedron");
    let scaled = points.iter().map(|&p| Vec3::from(p) * scale);
    scaled
        .map(|p| beyond_faces(hull, p))
        .fold(f32::MIN, f32::max)
}

fn slack(scale: Vec3) -> f32 {
    1e-5 * 2.0 * scale.abs().max_element()
}

#[test]
fn the_off_centre_cylinder_keeps_its_caps_at_the_scale_1195_reported() {
    let scale = Vec3::splat(999.9999);
    let worst = worst_outside(&off_centre_cylinder(), scale);
    assert!(
        worst <= slack(scale),
        "a point lies {worst} outside the hull"
    );
}

#[test]
fn every_point_is_inside_the_hull_a_few_ulps_around_each_magnitude() {
    let points = off_centre_cylinder();
    let stretches = [
        Vec3::ONE,
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(-1.5, 1.0, 1.0),
    ];
    for magnitude in [1e-3f32, 1.0, 1e3] {
        for ulps in -8i32..=8 {
            let s = f32::from_bits((magnitude.to_bits() as i32 + ulps) as u32);
            for stretch in stretches {
                let scale = stretch * s;
                let worst = worst_outside(&points, scale);
                assert!(
                    worst <= slack(scale),
                    "{scale}: a point lies {worst} outside"
                );
            }
        }
    }
}
