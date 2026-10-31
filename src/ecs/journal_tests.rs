//! The World's change journal (#1631): what it records, once, and only while on.

use std::collections::BTreeSet;

use super::World;

fn world_of(names: &[&str]) -> World {
    let mut world = World::new();
    for name in names {
        world.spawn(name.to_string());
    }
    world
}

#[test]
fn off_by_default_it_records_nothing() {
    let mut world = world_of(&["A"]);
    world.set_name(1, "B".into());
    assert!(world.journal_take().is_none());
    world.journal_resume();
    world.journal_pause();
    world.set_name(1, "C".into());
    assert!(
        world.journal_take().is_none(),
        "paused records nothing either"
    );
}

#[test]
fn the_first_write_records_the_state_before_once() {
    let mut world = world_of(&["A", "B"]);
    world.journal_resume();
    world.set_name(1, "First".into());
    world.set_name(1, "Second".into());
    world.transform_mut(1).expect("live").position.x = 3.0;
    let created = world.spawn("New".into());
    world.despawn_all(&BTreeSet::from([2]));
    let taken = world.journal_take().expect("recorded");
    assert_eq!(taken.next_id, created, "the allocator before the create");
    let names: Vec<_> = taken
        .before
        .iter()
        .map(|(id, r)| (*id, r.as_ref().map(|r| r.doc.name.clone())))
        .collect();
    assert_eq!(
        names,
        [
            (1, Some("A".into())),
            (2, Some("B".into())),
            (created, None)
        ]
    );
    assert!(world.journal_take().is_none(), "taken once");
}

#[test]
fn a_restored_entity_takes_its_old_slot_and_id() {
    let mut world = world_of(&["A", "B", "C"]);
    let recorded = world.record(2).expect("live");
    world.despawn_all(&BTreeSet::from([2]));
    world.spawn("D".into());
    world.restore_entity(recorded.doc, recorded.seq);
    assert_eq!(world.ids(), [1, 2, 3, 4]);
    assert_eq!(world.ids_with_mesh(), Vec::<u32>::new());
    assert_eq!(world.name(2).map(|n| (*n).clone()), Some("B".into()));
}

#[test]
fn the_allocator_goes_back_but_never_below_a_live_id() {
    let mut world = world_of(&["A", "B", "C"]);
    world.restore_next_id(2);
    assert_eq!(world.next_id(), 4, "3 is live");
    world.despawn_all(&BTreeSet::from([2, 3]));
    world.restore_next_id(2);
    assert_eq!(world.next_id(), 2);
}
