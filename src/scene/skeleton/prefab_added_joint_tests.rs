//! A Joint added to a prefab instance names a bone through Play, a reload and an
//! apply to source (#1527): its `connected_body` sits inside an *added component*,
//! and the one ref walk remaps it like an id at a leaf.

use super::fixture::{connected, hero, joint_to, reload};
use crate::scene::authoring::defaults::default_rigidbody;
use crate::scene::prefab::apply::apply_instance_to_source;
use crate::scene::prefab::instantiate_prefab_linked;
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{
    list_instance_overrides, read_prefab_file, reimport_instance, save_prefab, Scene,
};

/// Two linked instances of `Group { Hero (skinned), Rope }`, the first one's Rope
/// given a Joint to its Hero's Joint1: (scene, roots, path).
fn jointed(tag: &str) -> (Scene, [u32; 2], String) {
    let mut author = Scene::new();
    let group = author.add_entity("Group".into());
    let hero = hero(&mut author, tag);
    let rope = author.add_entity("Rope".into());
    author.set_parent(hero, Some(group)).unwrap();
    author.set_parent(rope, Some(group)).unwrap();
    author.world.set_rigidbody(rope, Some(default_rigidbody()));
    let path = crate::test_temp::dir().join(format!("rusty_1527_{tag}.prefab"));
    let path = path.to_string_lossy().into_owned();
    save_prefab(&author, group, &path).unwrap();
    let prefab = read_prefab_file(&path).unwrap();
    let mut scene = Scene::new();
    let roots = [0, 1].map(|_| instantiate_prefab_linked(&mut scene, &prefab, None, &path));
    let bone = scene.find_bone(child(&scene, roots[0], "Hero"), "Joint1");
    let rope = child(&scene, roots[0], "Rope");
    joint_to(&mut scene, rope, bone);
    (scene, roots, path)
}

/// The child of instance `root` named `name`.
fn child(scene: &Scene, root: u32, name: &str) -> u32 {
    let kids = scene.world.children(root);
    kids.into_iter()
        .find(|&c| scene.world.name(c).is_some_and(|n| *n == name))
        .unwrap()
}

/// Whether instance `root`'s Rope is jointed to its own Hero's Joint1.
fn joined_to_own_bone(scene: &Scene, root: u32) -> bool {
    let bone = scene.find_bone(child(scene, root, "Hero"), "Joint1");
    bone.is_some() && connected(scene, child(scene, root, "Rope")) == bone
}

#[test]
fn an_added_joint_keeps_its_bone_through_play_and_reload() {
    let (mut scene, roots, path) = jointed("added_joint");
    let rope = child(&scene, roots[0], "Rope");
    let first = connected(&scene, rope);
    assert_eq!(
        list_instance_overrides(&scene, roots[0]).unwrap(),
        ["2:+/joint"]
    );

    SceneSnapshot::capture(&scene).restore(&mut scene);
    assert_ne!(connected(&scene, rope), first, "the bones respawned");
    assert!(joined_to_own_bone(&scene, roots[0]), "Stop remaps it");
    assert_eq!(
        list_instance_overrides(&scene, roots[0]).unwrap(),
        ["2:+/joint"]
    );

    let back = reload(&scene);
    assert!(joined_to_own_bone(&back, roots[0]), "a load remaps it");
    assert_eq!(
        list_instance_overrides(&back, roots[0]).unwrap(),
        ["2:+/joint"]
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_added_joint_applied_to_source_names_the_bone_locally() {
    let (mut scene, roots, path) = jointed("added_joint_apply");
    apply_instance_to_source(&mut scene, roots[0]).unwrap();
    let source = read_prefab_file(&path).unwrap();
    let rope = source.entities.iter().find(|e| e.name == "Rope").unwrap();
    let hero = source.entities.iter().find(|e| e.name == "Hero").unwrap();
    let joint = rope.joint.as_ref().expect("the joint reached the source");
    assert_eq!(
        joint.connected_body,
        Some(hero.id),
        "a local id, not a scene id"
    );
    assert_eq!(rope.bone_refs["/joint/connected_body"][0], "Joint1");

    reimport_instance(&mut scene, roots[1]).unwrap();
    for root in roots {
        assert!(joined_to_own_bone(&scene, root), "instance {root}");
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
    let _ = std::fs::remove_file(&path);
}
