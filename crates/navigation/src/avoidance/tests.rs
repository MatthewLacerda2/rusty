//! Solver-level ORCA tests: signed velocities, not only distances, so the
//! mutation run can tell a flipped sign from a working dodge.

use glam::Vec2;

use super::lp::{lp2, lp3, Line};
use super::*;

fn agent(x: f32, z: f32, vx: f32, vz: f32) -> AvoidanceAgent {
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

const DT: f32 = 1.0 / 60.0;

#[test]
fn lone_agent_keeps_its_preferred_velocity() {
    let out = solve(&[agent(0.0, 0.0, 1.5, -0.5)], DT);
    assert_eq!(out, vec![Vec2::new(1.5, -0.5)]);
}

#[test]
fn zero_dt_passes_preferred_through() {
    let a = [agent(0.0, 0.0, 1.0, 0.0), agent(1.0, 0.0, -1.0, 0.0)];
    assert_eq!(
        solve(&a, 0.0),
        vec![Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0)]
    );
}

#[test]
fn neighbour_out_of_range_is_ignored() {
    let a = [
        agent(0.0, 0.0, 1.0, 0.0),
        agent(NEIGHBOR_DISTANCE + 0.5, 0.0, -1.0, 0.0),
    ];
    assert_eq!(
        solve(&a, DT),
        vec![Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0)]
    );
}

#[test]
fn head_on_pair_sidesteps_to_opposite_sides_reciprocally() {
    let a = [agent(0.0, 0.0, 1.0, 0.0), agent(3.0, 0.0, -1.0, 0.0)];
    let out = solve(&a, DT);
    // Each keeps to its own right (RVO2's right-leg tie-break), mirrored exactly.
    assert!(out[0].y < -0.05, "A dodges toward -z: {:?}", out[0]);
    assert!(out[1].y > 0.05, "B dodges toward +z: {:?}", out[1]);
    assert!(
        (out[0] + out[1]).length() < 1e-5,
        "equal shares mirror: {out:?}"
    );
    assert!(
        out[0].x > 0.0 && out[1].x < 0.0,
        "both still advance: {out:?}"
    );
}

#[test]
fn more_important_agent_ignores_the_lesser_which_takes_the_whole_dodge() {
    let mut a = [agent(0.0, 0.0, 1.0, 0.0), agent(3.0, 0.0, -1.0, 0.0)];
    let equal = solve(&a, DT);
    a[0].priority = 10;
    a[1].priority = 90;
    let out = solve(&a, DT);
    assert_eq!(out[0], Vec2::new(1.0, 0.0), "priority 10 never yields");
    assert!(
        out[1].y > equal[1].y + 0.05,
        "priority 90 dodges more: {out:?} vs {equal:?}"
    );
}

#[test]
fn non_solving_neighbour_is_avoided_with_full_responsibility() {
    let mut a = [agent(0.0, 0.0, 1.0, 0.0), agent(3.0, 0.0, -1.0, 0.0)];
    let equal = solve(&a, DT);
    a[1].solves = false;
    let out = solve(&a, DT);
    assert_eq!(
        out[1],
        Vec2::new(-1.0, 0.0),
        "a non-solver keeps its velocity"
    );
    assert!(
        out[0].y < equal[0].y - 0.05,
        "the solver dodges alone: {out:?}"
    );
}

#[test]
fn overlapping_agents_are_pushed_apart_within_one_step() {
    let a = [agent(0.0, 0.0, 0.0, 0.0), agent(0.5, 0.0, 0.0, 0.0)];
    let out = solve(&a, DT);
    assert!(out[0].x < -0.1, "left one moves left: {:?}", out[0]);
    assert!(out[1].x > 0.1, "right one moves right: {:?}", out[1]);
}

#[test]
fn neighbours_are_closest_first_and_capped() {
    let mut agents: Vec<AvoidanceAgent> = (0..14)
        .map(|i| agent(i as f32 * 0.5, 0.0, 0.0, 0.0))
        .collect();
    agents.push(agent(-0.25, 0.0, 0.0, 0.0));
    let grid = build_grid(&agents);
    let near = neighbors(&agents, &grid, 0);
    assert_eq!(near.len(), MAX_NEIGHBORS);
    assert_eq!(&near[..3], &[14, 1, 2], "closest first");
    assert!(!near.contains(&0), "never itself");
}

#[test]
fn neighbours_reach_across_grid_cells() {
    let a = [
        agent(NEIGHBOR_DISTANCE - 0.5, 0.0, 0.0, 0.0),
        agent(NEIGHBOR_DISTANCE + 0.5, 0.0, 0.0, 0.0),
    ];
    let grid = build_grid(&a);
    assert_eq!(neighbors(&a, &grid, 0), vec![1]);
    assert_eq!(neighbors(&a, &grid, 1), vec![0]);
}

fn line(px: f32, pz: f32, dx: f32, dz: f32) -> Line {
    Line {
        point: Vec2::new(px, pz),
        direction: Vec2::new(dx, dz),
    }
}

#[test]
fn lp2_projects_the_optimum_onto_a_violated_line() {
    // Allowed: left of the +x line through z=0.5, i.e. z >= 0.5.
    let (v, done) = lp2(&[line(0.0, 0.5, 1.0, 0.0)], 2.0, Vec2::new(1.0, 0.0), false);
    assert_eq!(done, 1);
    assert_eq!(v, Vec2::new(1.0, 0.5));
}

#[test]
fn lp2_clamps_an_unconstrained_optimum_to_the_speed_disc() {
    let (v, done) = lp2(&[], 1.0, Vec2::new(3.0, 4.0), false);
    assert_eq!(done, 0);
    assert!((v - Vec2::new(0.6, 0.8)).length() < 1e-6, "{v:?}");
}

#[test]
fn lp3_minimises_the_worst_violation_when_infeasible() {
    // z >= 0.5 and z <= -0.5 can't both hold: the fair compromise is z = 0.
    let lines = [line(0.0, 0.5, 1.0, 0.0), line(0.0, -0.5, -1.0, 0.0)];
    let (v, done) = lp2(&lines, 2.0, Vec2::new(1.0, 0.0), false);
    assert_eq!(done, 1, "the second line is infeasible");
    let fixed = lp3(&lines, done, 2.0, v);
    assert!(fixed.y.abs() < 1e-5, "splits the difference: {fixed:?}");
}
