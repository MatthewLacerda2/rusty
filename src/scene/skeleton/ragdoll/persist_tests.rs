//! A ragdoll survives a save/load (#466): bones are rebuilt from the model, and
//! each bone's Rigidbody and Joint come back on it by name — the joint's connected
//! bone too, though every bone has a fresh id.

use super::super::fixture::hero;
use super::*;
use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};

/// `j` without its connected body, an id that changes across a load.
fn shape(j: &JointComponent) -> JointComponent {
    JointComponent {
        connected_body: None,
        ..j.clone()
    }
}

#[test]
fn a_saved_ragdoll_comes_back_on_the_same_bones() {
    let mut scene = Scene::new();
    let owner = hero(&mut scene, "ragdoll_reload");
    let all = HitboxOptions {
        min_size: 0.0,
        ..HitboxOptions::default()
    };
    scene.generate_hitboxes(owner, &all).unwrap();
    let made = scene
        .build_ragdoll(owner, &RagdollOptions::default())
        .unwrap();
    assert_eq!(made.len(), 2, "both fixture joints are bodies: {made:?}");
    let child = scene.find_bone(owner, "Joint1").unwrap();
    scene.world.joint_mut(child).unwrap().swing_limit = 12.0; // a hand tweak
    let joint = scene.world.joint(child).unwrap().clone();
    let mass = scene.world.rigidbody(child).unwrap().mass;

    let data = to_scene_data(&scene);
    let saved = data.entities.iter().find(|e| e.name == "Hero").unwrap();
    let bodies = &saved.mesh.as_ref().unwrap().skeleton.bodies;
    assert_eq!(bodies.len(), 2);
    let saved_joint = bodies["Joint1"].joint.as_ref().unwrap();
    assert_eq!(saved_joint.connected_body, None, "no bone id is saved");
    assert_eq!(bodies["Joint1"].connected_bone.as_deref(), Some("Joint0"));

    let json = serde_json::to_string(&data).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(
        &mut loaded,
        serde_json::from_str::<SceneData>(&json).unwrap(),
    );
    let owner = loaded.find_entity_by_name("Hero").unwrap();
    let (root, child) = (
        loaded.find_bone(owner, "Joint0").unwrap(),
        loaded.find_bone(owner, "Joint1").unwrap(),
    );
    let back = loaded.world.joint(child).unwrap().clone();
    assert_eq!(back.connected_body, Some(root), "re-bound by bone name");
    assert_eq!(shape(&back), shape(&joint), "the tweak survived");
    assert_eq!(loaded.world.rigidbody(child).unwrap().mass, mass);
    assert!(loaded.world.joint(root).is_none());
    assert_eq!(loaded.ragdoll_bones(owner), vec![root, child]);
    let resaved = serde_json::to_string(&to_scene_data(&loaded)).unwrap();
    assert_eq!(resaved, json, "a load-save cycle changes nothing");
}
