//! A linked instance's reference to a bone is no override after Play (#1527): Stop
//! respawns the bones with fresh ids, and propagation rebuilds each baseline on them.

use super::fixture::{connected, hero};
use crate::components::JointComponent;
use crate::scene::authoring::defaults::default_rigidbody;
use crate::scene::prefab::{instantiate_prefab_linked, write_prefab_file};
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{
    list_instance_overrides, read_prefab_file, reimport_instance, save_prefab, Scene,
};

/// `Group { Hero (skinned), Rope jointed to Hero's Joint1 }` saved as a prefab.
fn write_prefab(tag: &str) -> String {
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
    // A Joint requires a Rigidbody (a load adds one; the prefab carries it).
    author.world.set_rigidbody(rope, Some(default_rigidbody()));
    let path = crate::test_temp::dir().join(format!("rusty_1527_{tag}.prefab"));
    let path = path.to_string_lossy().into_owned();
    save_prefab(&author, group, &path).unwrap();
    path
}

/// The child of instance `root` named `name`.
fn child(scene: &Scene, root: u32, name: &str) -> u32 {
    let kids = scene.world.children(root);
    kids.into_iter()
        .find(|&c| scene.world.name(c).is_some_and(|n| *n == name))
        .unwrap()
}

#[test]
fn a_joint_to_a_bone_is_no_override_after_play() {
    let path = write_prefab("stop_bone");
    let prefab = read_prefab_file(&path).unwrap();
    let mut scene = Scene::new();
    let roots = [0, 1].map(|_| instantiate_prefab_linked(&mut scene, &prefab, None, &path));
    let first_bone = scene.find_bone(child(&scene, roots[0], "Hero"), "Joint1");

    SceneSnapshot::capture(&scene).restore(&mut scene);
    let hero = child(&scene, roots[0], "Hero");
    assert_ne!(
        scene.find_bone(hero, "Joint1"),
        first_bone,
        "the bones respawned"
    );
    for root in roots {
        let rope = child(&scene, root, "Rope");
        let hero = child(&scene, root, "Hero");
        assert_eq!(connected(&scene, rope), scene.find_bone(hero, "Joint1"));
        let listed = list_instance_overrides(&scene, root).unwrap();
        assert!(listed.is_empty(), "{listed:?}");
    }

    // A later change to the joint in the source reaches both instances.
    let mut source = read_prefab_file(&path).unwrap();
    let rope = source
        .entities
        .iter_mut()
        .find(|e| e.name == "Rope")
        .unwrap();
    rope.joint.as_mut().unwrap().break_force = 42.0;
    write_prefab_file(&source, &path).unwrap();
    for root in roots {
        reimport_instance(&mut scene, root).unwrap();
        let rope = child(&scene, root, "Rope");
        assert_eq!(scene.world.joint(rope).unwrap().break_force, 42.0);
        let hero = child(&scene, root, "Hero");
        assert_eq!(connected(&scene, rope), scene.find_bone(hero, "Joint1"));
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
    let _ = std::fs::remove_file(&path);
}
