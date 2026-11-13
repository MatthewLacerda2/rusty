//! crates/ecs/src/changes/epoch_tests.rs — a cursor read against a World it was not opened
//! on reads `Rebuild` (#1777). Within one World a clear or an Undo step already
//! lifts every log's floor past an older cursor; across Worlds (a scene replaced
//! under a consumer that keeps its cursor) only the epoch tells the two apart.

use crate::{Changes, World};
use rusty_components::TransformComponent;

fn world_of(n: usize) -> World {
    let mut w = World::new();
    for i in 0..n {
        w.spawn(format!("e{i}"));
    }
    w
}

/// Both logs record, and the foreign cursor's revision sits inside the other
/// World's log, so the log alone would answer it with ids that are not its.
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
