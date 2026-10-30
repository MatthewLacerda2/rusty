//! The probe-array slot table (#1206): reuse, growth, eviction and the budget.

use super::{SlotTable, MAX_REFLECTION_PROBES};

/// Room for every slot: only growth and the half rule resize.
const ROOM: usize = MAX_REFLECTION_PROBES;

#[test]
fn a_first_plan_holds_exactly_what_it_needs_and_copies_everything() {
    let mut table = SlotTable::default();
    let plan = table.plan(&["a", "b", "c"], ROOM);
    assert_eq!(plan.capacity, 3);
    assert_eq!(plan.slot_of, vec![0, 1, 2]);
    assert_eq!(plan.copies, vec![0, 1, 2]);
}

#[test]
fn a_resident_cube_keeps_its_slot_and_is_not_copied_again() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c"], ROOM);
    let plan = table.plan(&["c", "a", "b"], ROOM);
    assert_eq!(plan.slot_of, vec![2, 0, 1]);
    assert!(plan.copies.is_empty());
}

#[test]
fn a_new_cube_takes_a_slot_no_wanted_cube_holds() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c", "d"], ROOM);
    // `b` left range; `e` arrives and must not overwrite `a`, `c` or `d`.
    let plan = table.plan(&["a", "e", "c", "d"], ROOM);
    assert_eq!(plan.slot_of, vec![0, 1, 2, 3]);
    assert_eq!(plan.copies, vec![1]);
    assert_eq!(plan.capacity, 4);
}

#[test]
fn outgrowing_the_array_recopies_every_slot() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"], ROOM);
    let plan = table.plan(&["a", "b", "c"], ROOM);
    assert_eq!(plan.capacity, 3);
    assert_eq!(plan.copies, vec![0, 1, 2]);
}

#[test]
fn the_array_shrinks_when_the_budget_or_the_wanted_count_drops() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c", "d"], ROOM);
    let plan = table.plan(&["a", "b", "c"], 3);
    assert_eq!(
        (plan.capacity, plan.copies.len()),
        (3, 3),
        "the budget dropped"
    );
    let plan = table.plan(&["a", "b"], 3);
    assert_eq!(plan.capacity, 3, "one slot spare is kept");
    let plan = table.plan(&["a"], 3);
    assert_eq!(plan.capacity, 1, "half empty: shrunk");
}

#[test]
fn a_forgotten_path_is_copied_again() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"], ROOM);
    table.forget(|p| p == "b");
    let plan = table.plan(&["a", "b"], ROOM);
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
    let plan = table.plan(&wanted, ROOM);
    assert_eq!(plan.capacity, MAX_REFLECTION_PROBES);
    assert_eq!(plan.slot_of.len(), MAX_REFLECTION_PROBES);
}
