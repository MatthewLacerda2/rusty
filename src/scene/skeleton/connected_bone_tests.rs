//! A ragdoll joint to a bone of its own skeleton keeps that bone by its chain
//! (#1277), so a load and a live re-import send a joint to a dropped bone to the
//! same place: the nearest surviving ancestor.

use super::fixture::{armature_owner, chain_skin, connected, joint_to, reimport_to};
use crate::components::mesh::BoneBody;
use crate::scene::serialize::to_scene_data;
use crate::scene::Scene;

/// An armature rig whose `head` bone is jointed to `spine`.
fn jointed_rig(scene: &mut Scene) -> u32 {
    let rig = armature_owner(scene);
    scene.sync_skeleton(rig);
    let (spine, head) = (
        scene.find_bone(rig, "spine").unwrap(),
        scene.find_bone(rig, "head").unwrap(),
    );
    let body = crate::scene::authoring::default_rigidbody();
    scene.world.set_rigidbody(head, Some(body));
    joint_to(scene, head, Some(spine));
    rig
}

#[test]
fn a_saved_bone_joint_names_its_connected_bone_by_chain() {
    let mut scene = Scene::new();
    jointed_rig(&mut scene);
    let data = to_scene_data(&scene);
    let rig = data.entities.iter().find(|e| e.name == "Rig").unwrap();
    let bodies = &rig.mesh.as_ref().unwrap().skeleton.bodies;
    assert_eq!(bodies["head"].connected_bone, ["spine", "hips"]);
}

#[test]
fn a_load_and_a_live_reimport_joint_a_dropped_bone_to_the_same_ancestor() {
    // Live: the rig is re-imported without `spine`.
    let mut live = Scene::new();
    let rig = jointed_rig(&mut live);
    reimport_to(&mut live, rig, chain_skin(&["hips", "head"]));
    let head = live.find_bone(rig, "head").unwrap();
    assert_eq!(connected(&live, head), live.find_bone(rig, "hips"));

    // Load: the saved body comes back onto a rig that lacks `spine`.
    let mut saved = Scene::new();
    let rig = jointed_rig(&mut saved);
    let data = to_scene_data(&saved);
    let doc = data.entities.iter().find(|e| e.name == "Rig").unwrap();
    let body: BoneBody = doc.mesh.as_ref().unwrap().skeleton.bodies["head"].clone();
    let mut loaded = Scene::new();
    let rig_again = armature_owner(&mut loaded);
    assert_eq!(rig_again, rig);
    let mut mesh = loaded.world.mesh_mut(rig).unwrap();
    mesh.skin = Some(chain_skin(&["hips", "head"]));
    mesh.skeleton.bodies.insert("head".into(), body);
    drop(mesh);
    loaded.sync_skeleton(rig);
    let head = loaded.find_bone(rig, "head").unwrap();
    assert_eq!(connected(&loaded, head), loaded.find_bone(rig, "hips"));
}
