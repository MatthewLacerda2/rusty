//! What one environment query costs on the Dust2-sized bench level (#1892)
//! ([`super::bench`]): the gate run (`n = 5`) checks the query works at level
//! scale and that its one flood stays local.

use glam::Vec3;

use super::bench::{level, query, query_far};
use rusty_navigation::ALL_AREAS;

#[test]
fn a_far_bot_reaches_the_ground_spots_and_not_the_upper_storey() {
    let (scene, nav, physics) = level(5, 0.25);
    let found = query_far(&scene, &nav, &physics);
    assert!(!found.is_empty(), "the ground spots are reachable");
    assert!(found.iter().all(|i| i.position.y < 1.0), "{found:?}");
}

#[test]
fn a_query_on_the_bench_level_finds_spots_and_floods_locally() {
    let (scene, nav, physics) = level(5, 1.0);
    let found = query(&scene, &nav, &physics);
    assert!(found > 0 && found < 64, "some spots, not all: {found}");
    let flood = nav
        .flood(Vec3::new(35.0, 0.0, 24.0), 26.0, ALL_AREAS)
        .expect("on the navmesh");
    assert!(
        flood.reached() * 2 < nav.spans.len(),
        "{} of {}",
        flood.reached(),
        nav.spans.len()
    );
}
