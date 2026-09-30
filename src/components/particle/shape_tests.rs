//! Emission-shape sampling lands where each shape claims.

use super::*;

const N: usize = 500;

fn samples(shape: EmitShape, from: EmitFrom) -> Vec<(Vec3, Vec3)> {
    let mut rng = Random::new(9);
    (0..N)
        .map(|_| shape.sample(from, Vec3::Y, &mut rng))
        .collect()
}

#[test]
fn sphere_volume_and_surface_launch_outward() {
    for (o, d) in samples(EmitShape::Sphere { radius: 2.0 }, EmitFrom::Volume) {
        assert!(o.length() <= 2.0 + 1e-4);
        assert!((d.length() - 1.0).abs() < 1e-4);
    }
    for (o, _) in samples(EmitShape::Sphere { radius: 2.0 }, EmitFrom::Surface) {
        assert!((o.length() - 2.0).abs() < 1e-3);
    }
}

#[test]
fn hemisphere_stays_on_the_axis_side() {
    for (o, d) in samples(EmitShape::Hemisphere { radius: 1.0 }, EmitFrom::Volume) {
        assert!(o.y >= -1e-5 && d.y >= -1e-5, "{o} {d}");
    }
}

#[test]
fn box_fills_and_skins_its_extents() {
    let size = Vec3::new(2.0, 4.0, 6.0);
    for (o, d) in samples(EmitShape::Box { size }, EmitFrom::Surface) {
        assert!(o.abs().cmple(size * 0.5 + 1e-5).all());
        let on_face = (0..3).any(|i| (o[i].abs() - size[i] * 0.5).abs() < 1e-5);
        assert!(on_face, "{o} is not on a face");
        assert_eq!(d, Vec3::Y);
    }
}

#[test]
fn cone_directions_stay_within_the_angle() {
    let shape = EmitShape::Cone {
        angle: 30.0,
        radius: 0.5,
    };
    let limit = 30f32.to_radians().cos() - 1e-4;
    for (o, d) in samples(shape, EmitFrom::Volume) {
        assert!(d.dot(Vec3::Y) >= limit, "{d} leaves the cone");
        assert!(o.y.abs() < 1e-5 && o.length() <= 0.5 + 1e-5);
    }
    for (o, d) in samples(shape, EmitFrom::Surface) {
        assert!((d.dot(Vec3::Y) - 30f32.to_radians().cos()).abs() < 1e-3);
        assert!((o.length() - 0.5).abs() < 1e-3);
        assert!(d.dot(o) > 0.0, "rim particles lean outward");
    }
}

#[test]
fn circle_edge_launches_outward_in_plane() {
    for (o, d) in samples(EmitShape::Circle { radius: 3.0 }, EmitFrom::Surface) {
        assert!((o.length() - 3.0).abs() < 1e-3 && o.y.abs() < 1e-5);
        assert!(d.y.abs() < 1e-5 && (d - o / 3.0).length() < 1e-3);
    }
}

#[test]
fn names_round_trip() {
    for name in EmitShape::NAMES {
        let s = EmitShape::from_parts(name, 1.0, 10.0, Vec3::ONE).unwrap();
        assert_eq!(s.name(), name);
    }
    assert_eq!(EmitShape::from_parts("torus", 1.0, 1.0, Vec3::ONE), None);
}
