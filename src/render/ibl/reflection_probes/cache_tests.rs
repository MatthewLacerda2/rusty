//! The probe-cube cache's eviction (#1584): least recently resident first, never a
//! resident cube, down to the budget.

use super::CubeCache;
use crate::render::ibl::reflection_probes::frame::array_bytes;

#[test]
fn the_least_recently_resident_cubes_go_first_down_to_the_budget() {
    let mut cache = CubeCache::default();
    for p in ["a", "b", "c", "d"] {
        cache.loaded(p, 4);
    }
    let cube = array_bytes(4, 1);
    assert!(cache.evict(&["a", "b", "c", "d"], 4 * cube).is_empty());
    // `b` and `d` were resident last; `a` before `c`.
    cache.evict(&["a"], 4 * cube);
    cache.evict(&["c"], 4 * cube);
    let evicted = cache.evict(&["b", "d"], 2 * cube);
    assert_eq!(evicted, vec!["a".to_string(), "c".to_string()]);
    assert_eq!(cache.held_bytes(), 2 * cube);
}

#[test]
fn a_resident_cube_is_kept_past_the_budget_and_its_size_remembered() {
    let mut cache = CubeCache::default();
    cache.loaded("big", 512);
    cache.loaded("small", 4);
    assert_eq!(cache.evict(&["big"], 0), vec!["small".to_string()]);
    assert_eq!(cache.held_bytes(), array_bytes(512, 1), "the nearest stays");
    assert_eq!(cache.size("small"), Some(4), "known without a reload");
    cache.forget(|p| p == "small");
    assert_eq!(cache.size("small"), None, "a re-bake may change it");
}
