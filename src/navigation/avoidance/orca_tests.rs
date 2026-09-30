//! Exact-value tests for the ORCA half-plane (`orca_line`), one per branch, plus
//! the grid-cell and range boundaries. Expected values are the RVO2 formulas
//! worked by hand (float64) on asymmetric inputs, so no x/y term can cancel out.

use glam::Vec2;

use super::*;

const DT: f32 = 1.0 / 60.0;

fn at(x: f32, z: f32, vx: f32, vz: f32) -> AvoidanceAgent {
    AvoidanceAgent {
        position: Vec2::new(x, z),
        velocity: Vec2::new(vx, vz),
        preferred: Vec2::new(vx, vz),
        radius: 0.5,
        max_speed: 2.0,
        priority: 50,
        solves: true,
    }
}

fn assert_line(me: AvoidanceAgent, other: AvoidanceAgent, point: [f32; 2], dir: [f32; 2]) {
    let line = orca_line(&me, &other, 0.5, DT).expect("a line");
    let (p, d) = (Vec2::from(point), Vec2::from(dir));
    assert!(
        (line.point - p).length() < 1e-4,
        "point {:?} != {p:?}",
        line.point
    );
    assert!(
        (line.direction - d).length() < 1e-4,
        "direction {:?} != {d:?}",
        line.direction
    );
    assert!(
        (line.direction.length() - 1.0).abs() < 1e-5,
        "unit direction"
    );
}

#[test]
fn cut_off_circle_line_is_exact() {
    // Slow closing, far off: w points back from the cone's truncation circle.
    let (me, other) = (at(0.0, 0.0, 0.3, 0.2), at(3.0, 1.0, 0.1, 0.1));
    assert_line(me, other, [0.711_055, 0.326_479], [-0.294_086, 0.955_779]);
}

#[test]
fn left_leg_line_is_exact() {
    let (me, other) = (at(1.0, -1.0, 1.4, 1.5), at(3.5, 0.5, -0.6, -0.3));
    assert_line(me, other, [1.235_736, 1.632_908], [0.629_004, 0.777_402]);
}

#[test]
fn right_leg_line_is_exact() {
    let (me, other) = (at(1.0, -1.0, 1.5, 0.4), at(3.5, 0.5, -0.5, -0.2));
    assert_line(me, other, [1.519_941, 0.296_487], [-0.981_945, -0.189_167]);
}

#[test]
fn overlapping_line_resolves_within_one_step_exactly() {
    let (me, other) = (at(1.0, 2.0, 0.3, -0.1), at(1.6, 2.3, -0.1, 0.1));
    assert_line(me, other, [-8.611_689, -4.655_976], [-0.455_199, 0.890_390]);
}

#[test]
fn share_scales_only_the_correction() {
    let (me, other) = (at(1.0, -1.0, 1.5, 0.4), at(3.5, 0.5, -0.5, -0.2));
    let half = orca_line(&me, &other, 0.5, DT).expect("a line");
    let full = orca_line(&me, &other, 1.0, DT).expect("a line");
    let correction = half.point - me.velocity;
    assert!((full.point - (me.velocity + 2.0 * correction)).length() < 1e-5);
}

#[test]
fn coincident_agents_with_equal_velocity_give_no_line() {
    // rel_pos = 0 and rel_vel = 0: no direction to push along.
    assert!(orca_line(&at(1.0, 1.0, 0.2, 0.0), &at(1.0, 1.0, 0.2, 0.0), 0.5, DT).is_none());
}

#[test]
fn cells_floor_by_neighbour_distance_on_both_axes() {
    let n = NEIGHBOR_DISTANCE;
    assert_eq!(cell_of(Vec2::new(2.5 * n, -0.3 * n)), (2, -1));
    assert_eq!(cell_of(Vec2::new(-0.01, 0.999 * n)), (-1, 0));
    assert_eq!(cell_of(Vec2::new(1.5 * n, 3.2 * n)), (1, 3));
}

#[test]
fn neighbour_exactly_at_range_is_excluded() {
    let a = [at(0.0, 0.0, 0.0, 0.0), at(NEIGHBOR_DISTANCE, 0.0, 0.0, 0.0)];
    assert!(neighbors(&a, &build_grid(&a), 0).is_empty());
}

#[test]
fn diagonal_neighbour_cell_is_searched() {
    let n = NEIGHBOR_DISTANCE;
    let a = [
        at(0.9 * n, 0.9 * n, 0.0, 0.0),
        at(1.1 * n, 1.1 * n, 0.0, 0.0),
    ];
    let grid = build_grid(&a);
    assert_eq!(neighbors(&a, &grid, 0), vec![1]);
    assert_eq!(neighbors(&a, &grid, 1), vec![0]);
}
