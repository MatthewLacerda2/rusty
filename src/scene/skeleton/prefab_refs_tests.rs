//! A linked prefab instance keeps its references to a bone, and drops one to a
//! destroyed entity, through a save and a load and an apply-to-source (#1264).

use super::fixture::{connected, hero, reload};
use crate::components::JointComponent;
use crate::scene::prefab::apply::apply_instance_to_source;
use crate::scene::prefab::instantiate_prefab_linked;
use crate::scene::{read_prefab_file, save_prefab, Scene};

/// A linked instance of `Group { Hero (skinned), Rope jointed to Hero's Joint1 }`,
/// written to its own prefab file: (scene, instance root, its hero, its rope, path).
fn linked(tag: &str) -> (Scene, u32, u32, u32, String) {
    let mut author = Scene::new();
    let group = author.add_entity("Group".into());
    let hero = hero(&mut author, tag);
    let rope = author.add_entity("Rope".into());
    author.set_parent(hero, Some(group)).unwrap();
    author.set_parent(rope, Some(group)).unwrap();
    let joint = JointComponent {
        connected_body: author.find_bone(hero, "Joint1"),
        ..JointComponent::default()
    };
    author.world.set_joint(rope, Some(joint));
    let path = crate::test_temp::dir().join(format!("rusty_1264_{tag}.prefab"));
    let path = path.to_string_lossy().into_owned();
    save_prefab(&author, group, &path).unwrap();

    let mut scene = Scene::new();
    let prefab = read_prefab_file(&path).unwrap();
    let root = instantiate_prefab_linked(&mut scene, &prefab, None, &path);
    let named = |s: &Scene, n: &str| {
        let kids = s.world.children(root);
        kids.into_iter()
            .find(|&c| s.world.name(c).is_some_and(|x| *x == n))
    };
    let (hero, rope) = (
        named(&scene, "Hero").unwrap(),
        named(&scene, "Rope").unwrap(),
    );
    (scene, root, hero, rope, path)
}

fn override_of_in(scene: &Scene, rope: u32) -> serde_json::Value {
    crate::scene::prefab::live::entity_overrides(&scene.world, rope).modifications
        ["/joint/connected_body"]
        .clone()
}

#[test]
fn a_linked_instance_keeps_its_joint_to_a_bone_after_a_load() {
    let (scene, _, hero, rope, path) = linked("keeps");
    assert_eq!(connected(&scene, rope), scene.find_bone(hero, "Joint1"));
    let overrides = crate::scene::prefab::live::entity_overrides(&scene.world, rope);
    assert!(overrides.is_empty(), "{overrides:?}");

    let loaded = reload(&scene);
    let spine = loaded.find_bone(hero, "Joint1");
    assert!(spine.is_some());
    assert_eq!(connected(&loaded, rope), spine, "joined to the live bone");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn destroying_an_overridden_reference_leaves_no_dead_id_after_a_load() {
    let (mut scene, _, _, rope, path) = linked("dead_override");
    let post = scene.add_entity("Post".into());
    scene.world.joint_mut(rope).unwrap().connected_body = Some(post);
    assert_eq!(override_of_in(&scene, rope), post);

    scene.destroy_entity(post);
    assert!(override_of_in(&scene, rope).is_null());
    let loaded = reload(&scene);
    assert_eq!(connected(&loaded, rope), None, "held to the world");
    assert!(override_of_in(&loaded, rope).is_null(), "no dead id saved");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn applying_a_joint_to_another_bone_writes_the_bone_by_name() {
    let (mut scene, root, hero, rope, path) = linked("apply_bone");
    scene.world.joint_mut(rope).unwrap().connected_body = scene.find_bone(hero, "Joint0");
    apply_instance_to_source(&mut scene, root).unwrap();

    let source = read_prefab_file(&path).unwrap();
    let doc = source.entities.iter().find(|e| e.name == "Rope").unwrap();
    let hero_local = source
        .entities
        .iter()
        .find(|e| e.name == "Hero")
        .unwrap()
        .id;
    assert_eq!(doc.joint.as_ref().unwrap().connected_body, Some(hero_local));
    assert_eq!(doc.bone_refs["/joint/connected_body"], ["Joint0"]);
    assert_eq!(connected(&scene, rope), scene.find_bone(hero, "Joint0"));
    let _ = std::fs::remove_file(&path);
}
