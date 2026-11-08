//! The generators (#1892): how many points, where, in what order, and the
//! navmesh projection that drops points with no floor.

use glam::Vec3;

use super::generate::{items, raw_points};
use super::spec::Generator;
use super::test_world::Arena;

fn donut(rings: u32, per_ring: u32) -> Generator {
    Generator::Donut {
        center: Vec3::ZERO,
        inner: 2.0,
        outer: 6.0,
        rings,
        per_ring,
    }
}

#[test]
fn a_donut_makes_rings_from_inner_to_outer_starting_at_plus_x() {
    let points = raw_points(&donut(3, 4));
    assert_eq!(points.len(), 12);
    assert_eq!(donut(3, 4).count(), 12);
    let radii: Vec<f32> = points.iter().step_by(4).map(|p| p.length()).collect();
    assert_eq!(radii, vec![2.0, 4.0, 6.0]);
    assert!((points[0] - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5);
    assert!(
        (points[1] - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-5,
        "{:?}",
        points[1]
    );
    let one = raw_points(&donut(1, 1));
    assert_eq!(one, vec![Vec3::new(4.0, 0.0, 0.0)], "one ring sits midway");
}

#[test]
fn a_grid_covers_its_square_row_by_row() {
    let g = Generator::Grid {
        center: Vec3::new(1.0, 0.0, 1.0),
        half_size: 2.0,
        spacing: 1.0,
    };
    let points = raw_points(&g);
    assert_eq!((points.len(), g.count()), (25, 25));
    assert_eq!(points[0], Vec3::new(-1.0, 0.0, -1.0));
    assert_eq!(points[1], Vec3::new(0.0, 0.0, -1.0));
    assert_eq!(points[24], Vec3::new(3.0, 0.0, 3.0));
    let degenerate = Generator::Grid {
        center: Vec3::ZERO,
        half_size: 1.0,
        spacing: 0.0,
    };
    assert_eq!(degenerate.count(), 0);
    assert!(raw_points(&degenerate).is_empty());
}

#[test]
fn projection_lands_points_on_the_floor_and_drops_the_ones_off_it() {
    let arena = Arena::new(|_| {});
    let g = Generator::Points(vec![
        Vec3::new(1.2, 1.0, 1.3),
        Vec3::new(500.0, 0.0, 0.0),
        Vec3::new(3.5, 0.5, 3.5),
    ]);
    let got = items(&arena.nav, &g, 2.0);
    assert_eq!(got.len(), 2, "the far point has no floor: {got:?}");
    assert!(
        got.iter().all(|p| p.y.abs() < 1e-4),
        "on the floor: {got:?}"
    );
    assert!(
        (got[0].x - 1.2).abs() < 1e-4 && (got[1].x - 3.5).abs() < 1e-4,
        "in order"
    );
    assert!(
        items(&arena.nav, &g, 0.1).len() < 2,
        "a short reach drops the raised ones"
    );
}
