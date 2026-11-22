use crate::{Changes, World};
use rusty_components::TransformComponent;

fn world_of(n: usize) -> World {
    let mut w = World::new();
    for i in 0..n {
        w.spawn(format!("e{i}"));
    }
    w
}

#[test]
fn a_cursor_read_against_another_recording_world_reads_rebuild() {
    let first = world_of(3);
    let mut cursor = first.change_cursor::<TransformComponent>();
    let mut other = World::new();
    let _opened = other.change_cursor::<TransformComponent>();
    for i in 0..6 {
        other.spawn(format!("o{i}"));
    }
    assert_eq!(other.changes(&mut cursor), Changes::Rebuild);
    assert_eq!(
        other.changes(&mut cursor),
        Changes::Ids(vec![]),
        "then it is the other World's"
    );
}
