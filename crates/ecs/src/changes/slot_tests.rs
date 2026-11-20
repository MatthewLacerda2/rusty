//! crates/ecs/src/changes/slot_tests.rs — every channel's `Channel::SLOT` is its own, so
//! two channels never share a log, and a channel's log is found by it.

use super::{Activity, Channel, Children, Layer, StaticSet, WorldPose};
use crate::bundle::Core;
use crate::World;
use rusty_components::registry::first_class;
use rusty_components::{MaterialAsset, PrefabLink, ScriptComponent, TransformComponent};

macro_rules! registry_slots {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        [$((<$ty as Channel>::NAME, <$ty as Channel>::SLOT),)*]
    };
}

fn of<T: Channel>() -> (&'static str, usize) {
    (T::NAME, T::SLOT)
}

#[test]
fn slots_are_unique() {
    let mut all = vec![
        of::<StaticSet>(),
        of::<WorldPose>(),
        of::<Activity>(),
        of::<Layer>(),
        of::<Children>(),
        of::<Core>(),
        of::<String>(),
        of::<TransformComponent>(),
        of::<Vec<ScriptComponent>>(),
        of::<MaterialAsset>(),
        of::<PrefabLink>(),
    ];
    all.extend(first_class!(registry_slots));
    let mut slots: Vec<usize> = all.iter().map(|(_, s)| *s).collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(slots.len(), all.len(), "two channels share a slot: {all:?}");
}

#[test]
fn a_channel_opened_late_finds_its_own_log() {
    let mut w = World::new();
    let id = w.spawn("e".to_string());
    let mut pose = w.change_cursor::<WorldPose>();
    let mut transform = w.change_cursor::<TransformComponent>();
    w.transform_mut(id).expect("transform").position.x = 1.0;
    let names = w.change_stats().into_iter().map(|s| s.name);
    let names: Vec<_> = names.collect();
    assert_eq!(
        names,
        ["static_set", "world_pose", "transform"],
        "in opening order"
    );
    assert_eq!(w.changes(&mut transform), crate::Changes::Ids(vec![id]));
    assert_eq!(w.changes(&mut pose), crate::Changes::Ids(vec![id]));
}
