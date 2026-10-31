//! Undo of a create (#1631): the entity leaves through the scene's one despawn
//! door, and its id is retired for good, as Unity never hands an undone
//! object's id out again.

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::{to_scene_data, Scene};

fn create(scene: &mut Scene, name: &str) -> u32 {
    scene.begin_edits();
    let id = create_entity(scene, name, Some(Primitive::Box));
    scene.commit_edits(None);
    id
}

#[test]
fn an_undone_create_drops_the_scenes_own_references() {
    let mut scene = Scene::new();
    let id = create(&mut scene, "Crate");
    scene.selected_entity_id = Some(id);
    scene.persistent.insert(id);
    scene.pending_destroy.push(id);
    scene.lightmaps.entries.push(LightmapEntry {
        entity: id,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    scene.undo();
    assert!(!scene.world.contains(id));
    assert_eq!(scene.selected_entity_id, None);
    assert!(scene.persistent.is_empty() && scene.pending_destroy.is_empty());
    assert!(scene.lightmaps.entries.is_empty());
}

#[test]
fn an_undone_id_is_never_handed_out_again_and_redo_brings_it_back() {
    let mut scene = Scene::new();
    let first = create(&mut scene, "A");
    let saved = |s: &Scene| to_scene_data(s).next_entity_id;
    let after_create = saved(&scene);
    scene.undo();
    assert_eq!(saved(&scene), after_create, "the allocator never goes back");
    scene.redo();
    assert_eq!(
        scene.world.name(first).map(|n| (*n).clone()),
        Some("A".into())
    );
    scene.undo();
    let second = create(&mut scene, "B");
    assert_ne!(second, first, "the undone id stays retired");
}
