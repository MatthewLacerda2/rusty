use crate::World;
use rusty_components::TransformComponent;

#[test]
fn a_channel_is_quiet_until_a_stamp_lands() {
    let mut w = World::new();
    let id = w.spawn("e".to_string());
    let mut cursor = w.change_cursor::<TransformComponent>();
    assert!(w.quiet(&cursor), "nothing stamped since the cursor opened");
    w.transform_mut(id).expect("has a transform").position.x = 1.0;
    assert!(!w.quiet(&cursor), "a write stamped it");
    w.changes(&mut cursor);
    assert!(w.quiet(&cursor), "read: quiet again");
    w.rewind_changes();
    assert!(!w.quiet(&cursor), "an Undo step reads Rebuild next");
}

#[test]
fn a_cursor_of_another_world_is_never_quiet() {
    let first = World::new();
    let cursor = first.change_cursor::<TransformComponent>();
    let other = World::new();
    assert!(!other.quiet(&cursor), "the other World has no such log yet");
    let _opened = other.change_cursor::<TransformComponent>();
    assert!(!other.quiet(&cursor), "nor is its log the cursor's");
}
