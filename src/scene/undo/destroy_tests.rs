//! Undoing a Destroy brings the entity back as it was (#1631): lit by the last
//! bake, wearing the decals it owned (#1649), and a skinned one bound to its
//! restored bones.

use glam::Vec3;

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::decal::DecalSpec;
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::{to_scene_data, Scene};

fn document(scene: &Scene) -> String {
    serde_json::to_string(&to_scene_data(scene)).expect("a scene serializes")
}

fn destroy(scene: &mut Scene, id: u32) {
    scene.begin_edits();
    scene.destroy_entity(id);
    scene.commit_edits(None);
}

#[test]
fn an_undone_destroy_puts_the_lightmap_entry_back() {
    let mut scene = Scene::new();
    let ids: Vec<u32> = ["A", "B", "C"]
        .iter()
        .map(|n| create_entity(&mut scene, n, Some(Primitive::Box)))
        .collect();
    for (page, &entity) in (0u32..).zip(&ids) {
        scene.lightmaps.insert(LightmapEntry {
            entity,
            page,
            scale_offset: [1.0, 1.0, 0.0, 0.0],
        });
    }
    let before = document(&scene);
    destroy(&mut scene, ids[1]);
    assert!(scene.lightmaps.get(ids[1]).is_none());
    scene.undo();
    assert_eq!(
        document(&scene),
        before,
        "the wall comes back lit, in order"
    );
    scene.redo();
    assert!(
        scene.lightmaps.get(ids[1]).is_none(),
        "redo destroys it again"
    );
    scene.undo();
    assert_eq!(document(&scene), before);
}

#[test]
fn an_undone_destroy_of_a_skinned_entity_binds_its_skeleton() {
    let mut scene = Scene::new();
    let hero = crate::scene::skeleton::fixture::hero(&mut scene, "undo_1631");
    let bones = scene.world.mesh(hero).unwrap().skeleton.ids();
    assert!(!bones.is_empty(), "the fixture is skinned");
    destroy(&mut scene, hero);
    scene.undo();
    let bound = scene.world.mesh(hero).unwrap().skeleton.ids();
    assert_eq!(
        bound, bones,
        "bound to its restored bones, not in bind pose"
    );
}

/// An owned decal goes with its owner, comes back with the Undo in its place in
/// the blend order, and goes again with the Redo.
#[test]
fn an_undone_destroy_puts_its_decals_back_in_blend_order() {
    let mut scene = Scene::new();
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let mut stamp = |owner| {
        scene.spawn_decal(
            Vec3::ZERO,
            Vec3::Y,
            DecalSpec {
                owner,
                ..Default::default()
            },
        )
    };
    let ids = [stamp(None), stamp(Some(wall)), stamp(None)].map(Option::unwrap);
    let order = |scene: &Scene| scene.decals.iter().map(|d| d.id).collect::<Vec<_>>();
    destroy(&mut scene, wall);
    assert_eq!(order(&scene), [ids[0], ids[2]]);
    scene.undo();
    assert_eq!(order(&scene), ids);
    scene.redo();
    assert_eq!(order(&scene), [ids[0], ids[2]]);
    scene.undo();
    assert_eq!(order(&scene), ids);
}

/// The journal records the parent a destroy takes the child out of, so the Undo
/// puts both links back, never only the child's (#1777): a one-sided link would
/// keep the child lit when its parent is switched off.
#[test]
fn an_undone_destroy_of_a_child_puts_it_back_in_its_parents_children() {
    let mut scene = Scene::new();
    let parent = create_entity(&mut scene, "Parent", None);
    let child = create_entity(&mut scene, "Child", None);
    scene.set_parent(child, Some(parent)).expect("no cycle");
    destroy(&mut scene, child);
    scene.undo();
    assert_eq!(scene.world.parent_id(child), Some(parent));
    assert_eq!(scene.world.children(parent), [child]);
    scene.world.set_active(parent, false);
    assert!(!scene.world.is_active_in_hierarchy(child));
}

/// Undo puts a step's entities back in id order (#1805): a child despawned out of
/// its parent's list comes back listed once, and a child older than its parent
/// comes back before it and waits for it.
#[test]
fn an_undone_destroy_puts_both_sides_of_a_parent_link_back() {
    for child_first in [false, true] {
        let mut scene = Scene::new();
        let names = if child_first {
            ["Child", "Group"]
        } else {
            ["Group", "Child"]
        };
        let [a, b] = names.map(|n| scene.add_entity(n.to_string()));
        let (group, child) = if child_first { (b, a) } else { (a, b) };
        scene.set_parent(child, Some(group)).unwrap();
        let before = document(&scene);
        destroy(&mut scene, if child_first { group } else { child });
        scene.undo();
        assert_eq!(document(&scene), before, "child first: {child_first}");
        assert_eq!(scene.world.children(group), vec![child]);
    }
}
