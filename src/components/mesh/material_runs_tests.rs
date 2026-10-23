//! How material slots pair with a mesh's index runs (#1285), the rule the renderer
//! and the lightmap bake share.

use super::material_runs;

/// Unity's `sharedMaterials` (#1285): slot `i` draws run `i`, a material past the
/// last run draws that run again, a run past the last material is not drawn, and an
/// entity without a Material draws every run.
#[test]
fn material_slots_pair_with_runs_the_way_unity_does() {
    let runs = || vec![0..3, 3..6];
    assert_eq!(material_runs(runs(), Some(2)), [(0, 0..3), (1, 3..6)]);
    assert_eq!(material_runs(runs(), Some(1)), [(0, 0..3)]);
    assert_eq!(
        material_runs(runs(), Some(3)),
        [(0, 0..3), (1, 3..6), (2, 3..6)]
    );
    assert_eq!(material_runs(runs(), None), [(0, 0..3), (1, 3..6)]);
    assert!(material_runs(Vec::new(), Some(2)).is_empty());
}
