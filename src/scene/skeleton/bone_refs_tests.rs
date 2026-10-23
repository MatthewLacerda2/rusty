//! A reference from outside a skeleton to a bone survives a save, a load and a
//! re-import by the bone's name (#1264).

use glam::Vec3;

use super::fixture::{armature_owner, connected, hero, reload, rope_to};
use crate::components::{JointComponent, RectTransformComponent};
use crate::scene::authoring::rect_transform::set_world_anchor;
use crate::scene::serialize::to_scene_data;
use crate::scene::Scene;

#[test]
fn a_joint_to_a_bone_is_saved_by_name_and_rebound_on_load() {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, "joint_to_bone");
    let rope = rope_to(&mut scene, hero, "Joint1");

    let data = to_scene_data(&scene);
    let saved = data.entities.iter().find(|e| e.id == rope).unwrap();
    let joint = saved.joint.as_ref().unwrap();
    assert_eq!(joint.connected_body, Some(hero), "the skinned entity");
    assert_eq!(
        saved.bone_refs["/joint/connected_body"],
        ["Joint1", "Joint0"],
        "the bone, then its ancestors"
    );

    let loaded = reload(&scene);
    let bone = loaded.find_bone(hero, "Joint1").unwrap();
    assert_eq!(connected(&loaded, rope), Some(bone));
    let live = loaded.world.entity_document(rope).unwrap();
    assert!(live.bone_refs.is_empty(), "document-only");
}

#[test]
fn a_reimport_rebinds_a_joint_to_the_new_bone() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let rope = rope_to(&mut scene, rig, "spine");
    let old = connected(&scene, rope).unwrap();

    let hung = scene.take_skeleton(rig);
    assert_eq!(connected(&scene, rope), Some(rig), "parked on the rig");
    scene.restore_skeleton(rig, hung);
    let spine = scene.find_bone(rig, "spine").unwrap();
    assert_ne!(spine, old, "the bone was rebuilt");
    assert_eq!(connected(&scene, rope), Some(spine));
}

#[test]
fn a_marker_over_a_bone_follows_it_through_a_reimport_and_a_load() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let marker = scene.add_entity("Marker".to_string());
    let mut rect = RectTransformComponent::default();
    set_world_anchor(&mut rect, scene.find_bone(rig, "head"), Vec3::Y);
    scene.world.set_rect_transform(marker, Some(rect));
    let target = |s: &Scene| s.world.rect_transform(marker).unwrap().world_anchor.clone();

    let parked = scene.take_skeleton(rig);
    scene.restore_skeleton(rig, parked);
    let anchor = target(&scene).unwrap();
    assert_eq!(anchor.target, scene.find_bone(rig, "head"));
    assert!(!anchor.target_lost, "re-bound, not lost");
    assert!(scene.world.is_active(marker));

    let saved = to_scene_data(&scene);
    let doc = saved.entities.iter().find(|e| e.id == marker).unwrap();
    assert_eq!(
        doc.bone_refs["/rect_transform/world_anchor/target"][0],
        "head"
    );
}

#[test]
fn a_joint_between_two_skeletons_bones_survives_a_load() {
    let mut scene = Scene::new();
    let a = hero(&mut scene, "bone_to_bone_a");
    let b = hero(&mut scene, "bone_to_bone_b");
    let (from, to) = (scene.find_bone(a, "Joint1"), scene.find_bone(b, "Joint0"));
    let joint = JointComponent {
        connected_body: to,
        ..JointComponent::default()
    };
    scene.world.set_joint(from.unwrap(), Some(joint));

    let loaded = reload(&scene);
    let from = loaded.find_bone(a, "Joint1").unwrap();
    assert_eq!(connected(&loaded, from), loaded.find_bone(b, "Joint0"));
}

#[test]
fn a_joint_to_a_bone_the_rig_lost_goes_to_its_nearest_ancestor() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let rope = rope_to(&mut scene, rig, "head");
    let hung = scene.take_skeleton(rig);
    // The new rig drops `head`: `hips → spine` is left.
    let mut skin = scene.world.mesh(rig).unwrap().skin.clone().unwrap();
    skin.names.truncate(2);
    skin.local_bind.truncate(2);
    skin.parents.truncate(2);
    skin.bind_global.truncate(2);
    skin.inverse_bind.truncate(2);
    skin.joint_nodes.truncate(2);
    scene.world.mesh_mut(rig).unwrap().skin = Some(skin);

    scene.restore_skeleton(rig, hung);
    assert_eq!(connected(&scene, rope), scene.find_bone(rig, "spine"));
}

#[test]
fn a_joint_between_two_props_is_left_alone() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let post = scene.add_entity("Post".to_string());
    let rope = rope_to(&mut scene, rig, "spine");
    scene.world.joint_mut(rope).unwrap().connected_body = Some(post);
    let hung = scene.take_skeleton(rig);
    scene.restore_skeleton(rig, hung);
    assert_eq!(connected(&scene, rope), Some(post));
}
