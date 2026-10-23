//! A ragdoll joint to a bone of its own skeleton keeps that bone by its chain
//! (#1277), so a load and a live re-import send a joint to a dropped bone to the
//! same place: the nearest surviving ancestor.

use super::fixture::{armature_owner, chain_skin, connected, joint_to, reimport_to, warnings_of};
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

#[test]
fn a_loaded_joint_that_lands_on_its_own_bone_is_warned_inert() {
    // `spine` jointed to `head`; the model drops `head`, so the chain lands on
    // `spine`, the joint's own bone.
    let mut saved = Scene::new();
    let rig = armature_owner(&mut saved);
    saved.sync_skeleton(rig);
    let (spine, head) = (
        saved.find_bone(rig, "spine").unwrap(),
        saved.find_bone(rig, "head").unwrap(),
    );
    joint_to(&mut saved, spine, Some(head));
    let data = to_scene_data(&saved);
    let doc = data.entities.iter().find(|e| e.name == "Rig").unwrap();
    let body: BoneBody = doc.mesh.as_ref().unwrap().skeleton.bodies["spine"].clone();

    let mut loaded = Scene::new();
    let rig = armature_owner(&mut loaded);
    let mut mesh = loaded.world.mesh_mut(rig).unwrap();
    mesh.skin = Some(chain_skin(&["hips", "spine"]));
    mesh.skeleton.bodies.insert("spine".into(), body);
    drop(mesh);
    let warned = warnings_of(|| loaded.sync_skeleton(rig));
    let spine = loaded.find_bone(rig, "spine").unwrap();
    assert_eq!(connected(&loaded, spine), Some(spine));
    let about_head: Vec<_> = warned.iter().filter(|w| w.contains("'head'")).collect();
    assert_eq!(about_head.len(), 1, "warned once: {warned:?}");
    assert!(about_head[0].contains("inert"), "{warned:?}");
}

/// What a loaded `head` joint to `spine` warns once the model keeps neither
/// `spine` nor `hips`, so the chain lands on the skinned entity, given `rig_body`.
fn landed_on_the_rig(rig_body: bool) -> (u32, Vec<String>) {
    let mut saved = Scene::new();
    let rig = jointed_rig(&mut saved);
    let data = to_scene_data(&saved);
    let doc = data.entities.iter().find(|e| e.name == "Rig").unwrap();
    let body: BoneBody = doc.mesh.as_ref().unwrap().skeleton.bodies["head"].clone();

    let mut loaded = Scene::new();
    assert_eq!(armature_owner(&mut loaded), rig);
    if rig_body {
        let body = crate::scene::authoring::default_rigidbody();
        loaded.world.set_rigidbody(rig, Some(body));
    }
    let mut mesh = loaded.world.mesh_mut(rig).unwrap();
    mesh.skin = Some(chain_skin(&["root", "head"]));
    mesh.skeleton.bodies.insert("head".into(), body);
    drop(mesh);
    let warned = warnings_of(|| loaded.sync_skeleton(rig));
    let head = loaded.find_bone(rig, "head").unwrap();
    assert_eq!(connected(&loaded, head), Some(rig));
    let about: Vec<String> = warned
        .into_iter()
        .filter(|w| w.contains("'spine'"))
        .collect();
    assert_eq!(about.len(), 1, "warned once: {about:?}");
    (rig, about)
}

#[test]
fn a_loaded_joint_landing_on_a_body_says_what_it_constrains_to() {
    let (rig, warned) = landed_on_the_rig(true);
    let body = format!("constraining it to entity {rig}'s body");
    assert!(warned[0].contains(&body), "{warned:?}");
    let (_, warned) = landed_on_the_rig(false);
    assert!(
        warned[0].contains("inert"),
        "nothing above has a body: {warned:?}"
    );
}
