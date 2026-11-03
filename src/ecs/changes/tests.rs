//! src/ecs/changes/tests.rs — what a cursor reads (#1705): the writes it sees, the
//! events that make it rebuild, and a log that stays bounded at Dust2 scale.

use crate::components::{JointComponent, TransformComponent};
use crate::ecs::{Changes, World};

fn world_of(n: usize) -> (World, Vec<u32>) {
    let mut w = World::new();
    let ids = (0..n).map(|i| w.spawn(format!("e{i}"))).collect();
    (w, ids)
}

fn nudge(w: &mut World, id: u32, x: f32) {
    w.transform_mut(id).expect("live").position.x = x;
}

#[test]
fn a_cursor_reads_each_write_once_ascending_and_only_its_channel() {
    let (mut w, ids) = world_of(4);
    let mut joints = w.change_cursor::<JointComponent>();
    let mut moves = w.change_cursor::<TransformComponent>();
    w.set_joint(ids[3], Some(JointComponent::default()));
    w.set_joint(ids[1], Some(JointComponent::default()));
    let _ = w.joint_mut(ids[3]); // a borrow counts as a change
    w.set_joint(ids[1], None); // so does a detach
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![ids[1], ids[3]]));
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![]), "read once");
    assert_eq!(
        w.changes(&mut moves),
        Changes::Ids(vec![]),
        "another channel"
    );
}

#[test]
fn cursors_on_one_channel_read_at_their_own_pace() {
    let (mut w, ids) = world_of(3);
    let mut fast = w.change_cursor::<TransformComponent>();
    let mut slow = w.change_cursor::<TransformComponent>();
    nudge(&mut w, ids[0], 1.0);
    assert_eq!(w.changes(&mut fast), Changes::Ids(vec![ids[0]]));
    nudge(&mut w, ids[0], 2.0); // read by `fast` once already: logged again
    nudge(&mut w, ids[2], 2.0);
    assert_eq!(w.changes(&mut fast), Changes::Ids(vec![ids[0], ids[2]]));
    assert_eq!(w.changes(&mut slow), Changes::Ids(vec![ids[0], ids[2]]));
}

#[test]
fn whole_entity_writes_stamp_every_column_channel() {
    let (mut w, ids) = world_of(2);
    let mut joints = w.change_cursor::<JointComponent>();
    let born = w.spawn("born".into());
    w.despawn(ids[0]);
    let doc = w.entity_document(ids[1]).expect("live");
    w.replace_entity(doc);
    assert_eq!(
        w.changes(&mut joints),
        Changes::Ids(vec![ids[0], ids[1], born])
    );
}

#[test]
fn a_cleared_world_an_undo_restore_and_another_world_read_rebuild() {
    let (mut w, _) = world_of(2);
    let mut cursor = w.change_cursor::<TransformComponent>();
    w.clear();
    assert_eq!(w.changes(&mut cursor), Changes::Rebuild, "cleared (a load)");
    assert_eq!(
        w.changes(&mut cursor),
        Changes::Ids(vec![]),
        "then caught up"
    );

    let (mut w, ids) = world_of(2);
    let mut cursor = w.change_cursor::<TransformComponent>();
    let recorded = w.record(ids[1]).expect("live");
    w.despawn(ids[1]);
    let _ = w.changes(&mut cursor);
    w.restore_entity(recorded.doc, recorded.seq);
    assert_eq!(w.changes(&mut cursor), Changes::Rebuild, "an Undo");

    let (mut other, _) = world_of(1);
    assert_eq!(
        other.changes(&mut cursor),
        Changes::Rebuild,
        "another World"
    );
    let _ = ids;
}

#[test]
fn a_channel_never_opened_holds_nothing() {
    let (mut w, ids) = world_of(50);
    let _joints = w.change_cursor::<JointComponent>();
    for (i, &id) in ids.iter().enumerate() {
        nudge(&mut w, id, i as f32 + 1.0);
    }
    let stats = w.change_stats();
    assert!(stats
        .iter()
        .all(|(name, _)| !name.contains("TransformComponent")));
    assert_eq!(w.change_revision::<TransformComponent>(), 0);
}

#[test]
fn the_log_stays_bounded_at_dust2_scale_while_a_cursor_keeps_up() {
    const N: usize = 3000;
    let (mut w, ids) = world_of(N);
    let mut every_tick = w.change_cursor::<TransformComponent>();
    let mut idle = w.change_cursor::<TransformComponent>(); // opened, never read
    let cap = super::log::cap(N);
    for tick in 0..60 {
        for &id in &ids {
            nudge(&mut w, id, tick as f32 + 1.0);
        }
        match w.changes(&mut every_tick) {
            Changes::Ids(got) => assert_eq!(got.len(), N, "tick {tick}"),
            Changes::Rebuild => panic!("a cursor read every tick fell behind at {tick}"),
        }
        let held = w.change_stats().iter().map(|&(_, n)| n).max();
        assert!(held.unwrap_or(0) <= cap, "tick {tick}: {held:?} > {cap}");
    }
    assert_eq!(
        w.changes(&mut idle),
        Changes::Rebuild,
        "the log dropped its window"
    );
}

#[test]
fn with_no_reader_ever_a_channel_holds_one_entry_per_id() {
    let (mut w, ids) = world_of(1000);
    let _idle = w.change_cursor::<TransformComponent>();
    for tick in 0..100 {
        for &id in &ids {
            nudge(&mut w, id, tick as f32 + 1.0);
        }
    }
    let held: Vec<usize> = w.change_stats().iter().map(|&(_, n)| n).collect();
    assert!(held.iter().all(|&n| n <= ids.len()), "{held:?}");
}
