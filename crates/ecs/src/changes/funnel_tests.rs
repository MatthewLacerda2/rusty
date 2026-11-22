use crate::{Changes, World};
use rusty_components::{AnimatorComponent, JointComponent, LodGroupComponent, TransformComponent};

fn pair() -> (World, u32, u32) {
    let mut w = World::new();
    let plain = w.spawn("plain".into());
    let full = w.spawn("full".into());
    w.set_component(full, Some(JointComponent::default()));
    w.set_component(full, Some(AnimatorComponent::default()));
    (w, plain, full)
}

#[test]
fn component_mut_stamps_only_a_column_the_entity_holds() {
    let (mut w, plain, full) = pair();
    let mut joints = w.change_cursor::<JointComponent>();
    assert!(w.component_mut::<JointComponent>(plain).is_none());
    assert!(w.component_mut::<JointComponent>(full).is_some());
    assert!(w.component_mut::<JointComponent>(999).is_none());
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![full]));
}

#[test]
fn detaching_or_taking_a_missing_column_stamps_nothing() {
    let (mut w, plain, full) = pair();
    let mut joints = w.change_cursor::<JointComponent>();
    assert!(w.set_component::<JointComponent>(plain, None));
    assert!(w.take_component::<JointComponent>(plain).is_none());
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![]));
    assert!(w.take_component::<JointComponent>(full).is_some());
    assert!(w.set_component::<JointComponent>(plain, Some(JointComponent::default())));
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![plain, full]));
}

#[test]
fn with_mut_and_ref_stamps_only_the_held_mutable_column() {
    let (mut w, plain, full) = pair();
    let mut animators = w.change_cursor::<AnimatorComponent>();
    let mut joints = w.change_cursor::<JointComponent>();
    let mut transforms = w.change_cursor::<TransformComponent>();
    let none = w.with_mut_and_ref::<AnimatorComponent, JointComponent, _>(plain, |_, _| ());
    assert!(none.is_none(), "no animator: nothing borrowed");
    w.with_mut_and_ref::<TransformComponent, JointComponent, _>(plain, |_, j| assert!(j.is_none()));
    w.with_mut_and_ref::<AnimatorComponent, JointComponent, _>(full, |_, j| assert!(j.is_some()));
    assert_eq!(w.changes(&mut animators), Changes::Ids(vec![full]));
    assert_eq!(
        w.changes(&mut joints),
        Changes::Ids(vec![]),
        "read, never stamped"
    );
    assert_eq!(w.changes(&mut transforms), Changes::Ids(vec![plain]));
}

#[test]
fn the_despawn_walk_names_only_the_columns_each_holder_has() {
    let (mut w, plain, full) = pair();
    let joint = JointComponent {
        connected_body: Some(plain),
        ..JointComponent::default()
    };
    w.set_component(full, Some(joint));
    let mut joints = w.change_cursor::<JointComponent>();
    let mut lods = w.change_cursor::<LodGroupComponent>();
    w.despawn(plain);
    assert_eq!(w.changes(&mut joints), Changes::Ids(vec![full]));
    assert_eq!(
        w.changes(&mut lods),
        Changes::Ids(vec![]),
        "neither has a LOD group"
    );
}
