//! Stop restoring the Play snapshot rewinds the World under every cursor, so a
//! cursor opened during Play reads `Rebuild` (#1705).

use crate::snapshot::SceneSnapshot;
use crate::Scene;
use rusty_components::TransformComponent;
use rusty_ecs::Changes;

#[test]
fn stop_restoring_the_play_snapshot_reads_rebuild() {
    let mut scene = Scene::new();
    let id = scene.add_entity("e".into());
    let snapshot = SceneSnapshot::capture(&scene); // Play
    let mut cursor = scene.world.change_cursor::<TransformComponent>();
    scene.world.transform_mut(id).expect("live").position.x = 3.0;
    snapshot.restore(&mut scene); // Stop
    assert_eq!(scene.world.changes(&mut cursor), Changes::Rebuild);
    assert_eq!(scene.world.changes(&mut cursor), Changes::Ids(vec![]));
}
