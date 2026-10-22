//! The probe-array slot table (#1206): reuse, growth, eviction and the budget.

use super::{SlotTable, MAX_REFLECTION_PROBES};

#[test]
fn a_first_plan_grows_to_a_power_of_two_and_copies_everything() {
    let mut table = SlotTable::default();
    let plan = table.plan(&["a", "b", "c"]);
    assert_eq!(plan.capacity, 4);
    assert_eq!(plan.slot_of, vec![0, 1, 2]);
    assert_eq!(plan.copies, vec![0, 1, 2]);
}

#[test]
fn a_resident_cube_keeps_its_slot_and_is_not_copied_again() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c"]);
    let plan = table.plan(&["c", "a"]);
    assert_eq!(plan.slot_of, vec![2, 0]);
    assert!(plan.copies.is_empty());
}

#[test]
fn a_new_cube_takes_a_slot_no_wanted_cube_holds() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c", "d"]);
    // `b` left range; `e` arrives and must not overwrite `a`, `c` or `d`.
    let plan = table.plan(&["a", "e", "c", "d"]);
    assert_eq!(plan.slot_of, vec![0, 1, 2, 3]);
    assert_eq!(plan.copies, vec![1]);
    assert_eq!(plan.capacity, 4);
}

#[test]
fn outgrowing_the_array_recopies_every_slot() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"]);
    let plan = table.plan(&["a", "b", "c"]);
    assert_eq!(plan.capacity, 4);
    assert_eq!(plan.copies, vec![0, 1, 2]);
}

#[test]
fn a_forgotten_path_is_copied_again() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"]);
    table.forget(|p| p == "b");
    let plan = table.plan(&["a", "b"]);
    assert_eq!(plan.slot_of, vec![0, 1]);
    assert_eq!(plan.copies, vec![1]);
}

#[test]
fn the_plan_never_exceeds_the_budget() {
    let names: Vec<String> = (0..MAX_REFLECTION_PROBES + 5)
        .map(|i| format!("p{i}"))
        .collect();
    let wanted: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut table = SlotTable::default();
    let plan = table.plan(&wanted);
    assert_eq!(plan.capacity, MAX_REFLECTION_PROBES);
    assert_eq!(plan.slot_of.len(), MAX_REFLECTION_PROBES);
}
