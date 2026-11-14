//! Tests for the occupancy grid (#1737).

use glam::Vec3;

use super::{Occupancy, Solid, CELL};

/// A 10 × 10 m floor slab under a 1 m cube at its middle.
fn floor_and_cube() -> Occupancy {
    let solids = [
        Solid::cuboid(Vec3::new(-5.0, -0.5, -5.0), Vec3::new(5.0, 0.0, 5.0)),
        Solid::cuboid(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)),
    ];
    Occupancy::build(&solids, CELL).expect("something solid")
}

#[test]
fn a_convex_solid_is_filled_inside() {
    let occ = floor_and_cube();
    assert!(occ.is_solid(Vec3::new(0.1, 0.6, 0.1)), "inside the cube");
    assert!(occ.is_solid(Vec3::new(3.0, -0.2, 3.0)), "inside the slab");
    assert!(!occ.is_solid(Vec3::new(3.0, 1.0, 3.0)), "the air");
    assert!(
        !occ.is_solid(Vec3::new(50.0, 0.0, 0.0)),
        "off the grid is open"
    );
}

#[test]
fn clearance_grows_away_from_geometry() {
    let occ = floor_and_cube();
    assert_eq!(occ.clearance(Vec3::new(0.1, 0.5, 0.1)), 0.0);
    let near = occ.clearance(Vec3::new(1.2, 0.5, 0.1));
    let far = occ.clearance(Vec3::new(3.2, 2.7, 3.1));
    assert!(near > 0.0 && near < far, "{near} {far}");
}

#[test]
fn a_ray_stops_at_the_first_solid_and_escapes_open_sky() {
    let occ = floor_and_cube();
    let down = occ
        .ray(Vec3::new(3.0, 2.0, 3.0), Vec3::NEG_Y, 10.0)
        .expect("the floor");
    assert!((down - 2.0).abs() <= CELL, "{down}");
    let side = occ
        .ray(Vec3::new(-3.0, 0.5, 0.0), Vec3::X, 10.0)
        .expect("the cube");
    assert!((side - 2.5).abs() <= CELL, "{side}");
    assert_eq!(occ.ray(Vec3::new(3.0, 2.0, 3.0), Vec3::Y, 100.0), None);
    assert_eq!(
        occ.ray(Vec3::new(3.0, 0.5, 3.0), Vec3::X, 1.0),
        None,
        "past max"
    );
}

#[test]
fn solid_fraction_counts_the_boxs_cells() {
    let occ = floor_and_cube();
    let f = occ.solid_fraction(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!((f - 0.25).abs() < 0.01, "{f}");
    assert_eq!(occ.solid_fraction(Vec3::ZERO, Vec3::ZERO), 0.0);
}

/// A non-convex mesh marks its surface, not the space it wraps.
#[test]
fn a_mesh_solid_marks_only_its_surface() {
    let mut shell = Solid::cuboid(Vec3::splat(-3.0), Vec3::splat(3.0));
    shell.convex = false;
    let occ = Occupancy::build(&[shell], CELL).unwrap();
    assert!(occ.is_solid(Vec3::new(0.0, 2.9, 0.0)));
    assert!(!occ.is_solid(Vec3::ZERO));
}

/// A malformed solid (a NaN vertex, an index past the end) is skipped, not a panic.
#[test]
fn malformed_triangles_are_skipped() {
    let bad = Solid {
        vertices: vec![Vec3::ZERO, Vec3::NAN, Vec3::X],
        triangles: vec![[0, 1, 2], [0, 2, 9]],
        convex: true,
    };
    let good = Solid::cuboid(Vec3::ZERO, Vec3::ONE);
    let occ = Occupancy::build(&[bad.clone(), good], CELL).unwrap();
    assert!(occ.is_solid(Vec3::splat(0.5)));
    assert!(Occupancy::build(&[], CELL).is_none());
}

/// A level too wide for the cap is gridded coarser rather than failing.
#[test]
fn a_huge_level_is_gridded_coarser() {
    let slab = Solid::cuboid(Vec3::ZERO, Vec3::new(400.0, 4.0, 400.0));
    let occ = Occupancy::build_capped(&[slab], CELL, 100_000).unwrap();
    assert!(occ.cell > CELL);
    assert!(occ.dims.iter().product::<usize>() <= 100_000);
    assert!(occ.is_solid(Vec3::new(200.0, 2.0, 200.0)));
}

/// A box scaled so far its cell count overflows a `usize` is gridded coarser, not
/// wrapped to a small count; one whose span overflows a float is no grid.
#[test]
fn a_huge_scaled_box_does_not_overflow_the_count() {
    let huge = Solid::cuboid(Vec3::splat(-1.0e18), Vec3::splat(1.0e18));
    let occ = Occupancy::build_capped(&[huge], CELL, 100_000).unwrap();
    assert!(occ.dims.iter().product::<usize>() <= 100_000);
    let beyond = Solid::cuboid(Vec3::splat(-3.0e38), Vec3::splat(3.0e38));
    assert!(Occupancy::build_capped(&[beyond], CELL, 100_000).is_none());
}
