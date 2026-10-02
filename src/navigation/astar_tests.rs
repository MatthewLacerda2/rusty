use super::test_support::ground;
use super::*;
use glam::Vec3;

fn open(size: f32) -> NavigationGraph {
    NavigationGraph::new(0.0, size, 0.0, size, 1.0)
}

fn cells(path: &[SpanRef]) -> Vec<(i32, i32)> {
    path.iter().map(|s| (s.gx, s.gz)).collect()
}

/// A wrong branch returns `target` always; correct returns path[1], the adjacent cell.
#[test]
fn next_step_is_adjacent_not_the_goal() {
    let g = open(10.0);
    let step = g.get_next_path_step(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0));
    assert!((step.x - 1.0).abs() < 0.01, "expected (1,0), got {step:?}");
    assert!(step.z.abs() < 0.01, "expected (1,0), got {step:?}");
}

/// Priority-queue ordering mutations change the expansion and the path.
#[test]
fn find_path_exact_cardinal_sequence() {
    let g = open(5.0);
    let path = g.find_path(ground(&g, 0, 0), ground(&g, 3, 0)).unwrap();
    assert_eq!(cells(&path), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
}

/// A diagonal step (cost √2) must beat two cardinals (cost 2).
#[test]
fn find_path_single_diagonal_beats_two_cardinals() {
    let g = open(5.0);
    let path = g.find_path(ground(&g, 0, 0), ground(&g, 1, 1)).unwrap();
    assert_eq!(cells(&path), vec![(0, 0), (1, 1)]);
}

/// The boundary at exactly `max_step` passes; `max_step + ε` is rejected.
#[test]
fn link_boundary_at_max_step() {
    let mut g = open(5.0);
    let i = g.span_range(1, 0).start;
    g.spans[i].y = g.max_step;
    assert!(
        g.link_to(ground(&g, 0, 0), 1, 0).is_some(),
        "exact max_step"
    );
    g.spans[i].y += 0.001;
    assert!(
        g.link_to(ground(&g, 0, 0), 1, 0).is_none(),
        "above max_step"
    );
}

/// Lazy-deletion and reconstruction mutations break the exact diagonal sequence.
#[test]
fn find_path_three_diagonal_steps_exact_sequence() {
    let g = open(5.0);
    let path = g.find_path(ground(&g, 0, 0), ground(&g, 3, 3)).unwrap();
    assert_eq!(cells(&path), vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
}

/// A goal on a span nothing links to is unreachable.
#[test]
fn isolated_goal_has_no_path() {
    let mut g = open(5.0);
    let i = g.span_range(3, 3).start;
    g.spans[i].y = 5.0; // a pillar top
    assert!(g.find_path(ground(&g, 0, 0), ground(&g, 3, 3)).is_none());
    let target = Vec3::new(3.0, 5.0, 3.0);
    assert_eq!(g.get_next_path_step(Vec3::ZERO, target), target, "beeline");
}
