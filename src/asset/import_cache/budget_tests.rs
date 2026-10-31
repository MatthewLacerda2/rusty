//! The import cache's budget (#857): least recently used goes, a lone entry over
//! the budget stays.

use super::*;
use crate::asset::import_file;

fn model(name: &str) -> std::path::PathBuf {
    super::tests::model(name)
}

fn budget_cache(name: &str) -> (Cache, ImportedAsset, usize) {
    let asset = import_file(&model(name)).unwrap();
    let size = approx_bytes(&asset);
    assert!(size > 0);
    (Cache::default(), asset, size)
}

#[test]
fn past_the_budget_the_least_recently_used_goes() {
    let (mut cache, asset, size) = budget_cache("budget");
    let geometry = Geometry::of(&ImportSettings::default());
    // A budget of one besides the largest: two entries fit.
    for key in ["a", "b"] {
        cache.insert(key.into(), geometry, Vec::new(), asset.clone(), size);
    }
    assert!(cache.hit("a", geometry).is_some(), "a is used last");
    cache.insert("c".into(), geometry, Vec::new(), asset.clone(), size);
    let held: Vec<_> = cache.by_path.keys().cloned().collect();
    assert_eq!(held, ["a", "c"], "b, least recently used, went");
    assert_eq!(cache.bytes, 2 * size);
}

#[test]
fn a_lone_entry_over_the_budget_stays_beside_smaller_ones() {
    let (mut cache, asset, size) = budget_cache("over");
    let geometry = Geometry::of(&ImportSettings::default());
    let mut small = asset.clone();
    small.sub_meshes[0].indices.truncate(3);
    let small_size = approx_bytes(&small);
    assert!(small_size < size);
    let budget = small_size;
    cache.insert("big".into(), geometry, Vec::new(), asset, budget);
    assert!(size > budget, "big is over the budget on its own");
    for key in ["s1", "s2", "s1", "s2"] {
        cache.insert(key.into(), geometry, Vec::new(), small.clone(), budget);
        assert!(
            cache.hit("big", geometry).is_some(),
            "{key} did not evict big"
        );
    }
}
