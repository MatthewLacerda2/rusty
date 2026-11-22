use super::{clip_runs, material_runs};

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

#[test]
fn runs_clip_to_the_index_count() {
    let runs = [0..3, 3..9, 9..12];
    assert_eq!(clip_runs(runs.clone().into_iter(), 6), [0..3, 3..6]);
    assert_eq!(clip_runs(runs.clone().into_iter(), 12), runs);
}
