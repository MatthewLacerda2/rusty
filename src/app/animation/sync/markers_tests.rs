//! The marker arithmetic (#1896) at its edges: a follower missing the pair
//! falls back to normalized time, a repeated pair picks the one nearest, a
//! non-looping clip has no pair outside its markers, and a lone marker loops.

use super::markers::{place, position};
use super::rig::marks;

#[test]
fn a_follower_without_the_pair_takes_the_normalized_phase() {
    let leader = marks(&[("FootL", 0.0), ("FootR", 0.5)]);
    let pos = position(&leader, 1.0, 0.25, true);
    assert_eq!(pos.pair, Some(("FootL", "FootR", 0.5)));
    let other = marks(&[("Jump", 0.1), ("Land", 0.3)]);
    assert_eq!(place(&other, 2.0, pos, 0.0, true), 0.5);
    assert_eq!(place(&[], 2.0, pos, 0.0, true), 0.5);
}

#[test]
fn a_repeated_pair_matches_the_one_nearest() {
    let leader = marks(&[("FootL", 0.0), ("FootR", 0.5)]);
    let pos = position(&leader, 1.0, 0.25, true);
    let double = marks(&[
        ("FootL", 0.0),
        ("FootR", 0.5),
        ("FootL", 1.0),
        ("FootR", 1.5),
    ]);
    assert_eq!(place(&double, 2.0, pos, 0.1, true), 0.25);
    assert_eq!(place(&double, 2.0, pos, 1.3, true), 1.25);
}

#[test]
fn the_seam_pair_wraps_only_when_looping() {
    let run = marks(&[("FootL", 0.09), ("FootR", 0.39)]);
    let wrapped = position(&run, 0.6, 0.05, true);
    let (first, second, frac) = wrapped.pair.unwrap();
    assert_eq!((first, second), ("FootR", "FootL"));
    assert!((frac - (0.26 / 0.3)).abs() < 1e-5, "{frac}");
    assert_eq!(position(&run, 0.6, 0.05, false).pair, None);
    assert_eq!(position(&run, 0.6, 0.5, false).pair, None);
}

#[test]
fn a_lone_marker_loops_onto_itself() {
    let one = marks(&[("Plant", 0.2)]);
    let pos = position(&one, 1.0, 0.7, true);
    assert_eq!(pos.pair, Some(("Plant", "Plant", 0.5)));
    let other = marks(&[("Plant", 0.4)]);
    assert!((place(&other, 2.0, pos, 0.0, true) - 1.4).abs() < 1e-5);
}
