//! The probe-array slot table (#1206): reuse, spares, growth, eviction and the budget.

use super::{SlotTable, MAX_REFLECTION_PROBES, SPARE_SLOTS};

/// Room for every slot: only growth and the half rule resize.
const ROOM: usize = MAX_REFLECTION_PROBES;

#[test]
fn a_first_plan_holds_what_it_needs_and_the_spares_and_copies_what_it_needs() {
    let mut table = SlotTable::default();
    let plan = table.plan(&["a", "b", "c"], ROOM);
    assert_eq!(plan.capacity, 3 + SPARE_SLOTS);
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

/// Pacing across the budget's edge: the cube that left range waits in a spare slot
/// and comes back without a copy.
#[test]
fn a_cube_that_left_range_comes_back_from_a_spare_slot() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b", "c", "d"], ROOM);
    let plan = table.plan(&["a", "e", "c", "d"], ROOM);
    assert_eq!(plan.slot_of, vec![0, 4, 2, 3], "`e` takes an empty spare");
    for _ in 0..3 {
        assert!(table.plan(&["a", "b", "c", "d"], ROOM).copies.is_empty());
        assert!(table.plan(&["a", "e", "c", "d"], ROOM).copies.is_empty());
    }
}

#[test]
fn a_new_cube_overwrites_the_slot_wanted_least_recently() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"], ROOM); // slots a b _ _
    table.plan(&["c", "d"], ROOM); // a b c d: `a` and `b` are older
    table.plan(&["a", "d"], ROOM); // `b` is now the oldest
    let plan = table.plan(&["a", "e"], ROOM);
    assert_eq!(plan.slot_of, vec![0, 1], "`e` overwrites `b`, not `c`");
    assert!(table.plan(&["c", "e"], ROOM).copies.is_empty());
}

#[test]
fn outgrowing_the_array_recopies_every_slot() {
    let mut table = SlotTable::default();
    table.plan(&["a", "b"], ROOM);
    let plan = table.plan(&["a", "b", "c", "d", "e"], ROOM);
    assert_eq!(plan.capacity, 5 + SPARE_SLOTS);
    assert_eq!(plan.copies, vec![0, 1, 2, 3, 4]);
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
    let wanted: Vec<String> = (0..10).map(|i| format!("p{i}")).collect();
    let wanted: Vec<&str> = wanted.iter().map(String::as_str).collect();
    table.plan(&wanted, ROOM);
    let plan = table.plan(&wanted[..5], ROOM);
    assert_eq!(plan.capacity, 12, "half of it still needed: kept");
    let plan = table.plan(&wanted[..2], ROOM);
    assert_eq!(plan.capacity, 2 + SPARE_SLOTS, "under half: shrunk");
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
