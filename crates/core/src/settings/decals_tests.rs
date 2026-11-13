//! The `"decals"` block of `project.rusty` (#1517): a partial block keeps the
//! defaults, and what it can't read is named, never silently dropped.

use serde_json::json;

use super::DecalLimits;

#[test]
fn a_partial_block_keeps_the_defaults_for_what_it_omits() {
    let (limits, ignored) = DecalLimits::read_json(&json!({ "max_world": 12 }));
    let expected = DecalLimits {
        max_world: 12,
        ..DecalLimits::default()
    };
    assert_eq!((limits, ignored.len()), (expected, 0));
}

#[test]
fn a_misspelled_key_is_named_with_the_valid_ones() {
    let (limits, ignored) = DecalLimits::read_json(&json!({ "max_owner": 3 }));
    assert_eq!(limits, DecalLimits::default());
    assert_eq!(ignored.len(), 1);
    assert!(ignored[0].contains("'max_owner'"), "{ignored:?}");
    assert!(ignored[0].contains("max_per_owner, max_world, max_total"));
}

#[test]
fn a_value_that_is_not_a_whole_number_is_named_and_skipped() {
    let blob = json!({ "max_total": -1, "max_world": "lots", "max_per_owner": 8 });
    let (limits, ignored) = DecalLimits::read_json(&blob);
    assert_eq!(limits.max_per_owner, 8);
    assert_eq!(limits.max_total, DecalLimits::default().max_total);
    assert_eq!(ignored.len(), 2, "{ignored:?}");
}

#[test]
fn the_file_spelling_round_trips() {
    let limits = DecalLimits {
        max_per_owner: 1,
        max_world: 2,
        max_total: 3,
    };
    let text = serde_json::to_string(&limits).unwrap();
    assert_eq!(serde_json::from_str::<DecalLimits>(&text).unwrap(), limits);
}
