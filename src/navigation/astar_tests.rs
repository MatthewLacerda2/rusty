use super::*;
use glam::Vec3;

fn open(size: f32) -> NavigationGraph {
    NavigationGraph::new(0.0, size, 0.0, size, 1.0)
}

/// Kill "replace > with <" at get_next_path_step line 62: wrong branch returns
/// `target` always; correct returns path[1] (the adjacent cell).
#[test]
fn next_step_is_adjacent_not_the_goal() {
    let g = open(10.0);
    let step = g.get_next_path_step(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0));
    assert!((step.x - 1.0).abs() < 0.01, "expected (1,0), got {step:?}");
    assert!(step.z.abs() < 0.01, "expected (1,0), got {step:?}");
}

/// Kill priority-queue ordering mutations: wrong order changes A* expansion,
/// producing a path whose first interior node is not path[1] = (1,0).
#[test]
fn find_path_exact_cardinal_sequence() {
    let g = open(5.0);
    assert_eq!(
        g.find_path(0, 0, 3, 0).unwrap(),
        vec![(0, 0), (1, 0), (2, 0), (3, 0)]
    );
}

/// Kill cost `+dh → *dh` and heuristic `*1.414 → +1.414`: diagonal step
/// (cost 1.414) must beat two cardinals (cost 2.0).
#[test]
fn find_path_single_diagonal_beats_two_cardinals() {
    let g = open(5.0);
    assert_eq!(g.find_path(0, 0, 1, 1).unwrap(), vec![(0, 0), (1, 1)]);
}

/// Kill step_connected `<= → <`: the boundary at exactly max_step must pass,
/// and max_step + epsilon must be rejected.
#[test]
fn step_connected_boundary_at_max_step() {
    let mut g = NavigationGraph::new(0.0, 5.0, 0.0, 5.0, 1.0);
    let idx = g.index(1, 0);
    g.heightfield[idx] = g.max_step;
    assert!(g.step_connected(0, 0, 1, 0), "exact max_step connected");
    g.heightfield[idx] += 0.001;
    assert!(!g.step_connected(0, 0, 1, 0), "above max_step disconnected");
}

/// Kill `&& → ||` in step_connected slope guard: slope-only violation with
/// dh within max_step but above max_slope*run must still block the move.
#[test]
fn step_connected_slope_rule_overrides_step_limit() {
    let mut g = NavigationGraph::new(0.0, 5.0, 0.0, 5.0, 1.0);
    g.max_step = 2.0;
    g.max_slope = 0.5;
    let idx = g.index(1, 0);
    g.heightfield[idx] = 0.5; // = max_slope * run (1.0); both limits met
    assert!(g.step_connected(0, 0, 1, 0), "at slope limit: connected");
    g.heightfield[idx] = 0.6; // above slope limit but within max_step = 2.0
    assert!(
        !g.step_connected(0, 0, 1, 0),
        "slope violated: disconnected"
    );
}

/// Kill lazy-deletion `> → <` and path reconstruction mutations: the optimal
/// 3-diagonal path must be exactly 4 nodes with the right interior sequence.
#[test]
fn find_path_three_diagonal_steps_exact_sequence() {
    let g = open(5.0);
    let path = g.find_path(0, 0, 3, 3).unwrap();
    assert_eq!(path.len(), 4);
    assert_eq!(path[1], (1, 1));
    assert_eq!(path[2], (2, 2));
    assert_eq!(path[3], (3, 3));
}
