//! `Scene::skinned_ids` on a level of static meshes: one walk of the Mesh column,
//! no skeleton read, the rigs among them in insertion order, and a mesh seen the
//! moment it becomes skinned or stops being; `build_skin_palettes` re-posing a
//! rig into the palette it already holds.

use glam::Quat;

use super::probe;
use super::rig::armature_owner;
use crate::Scene;
use rusty_components::MeshComponent;
use rusty_ecs::world::walks;

fn static_mesh(scene: &mut Scene) -> u32 {
    let id = scene.add_entity("Prop".to_string());
    let mesh = MeshComponent::new("Asset", Vec::new(), Vec::new());
    scene.world.set_mesh(id, Some(mesh));
    id
}

fn statics(scene: &mut Scene, n: usize) -> Vec<u32> {
    (0..n).map(|_| static_mesh(scene)).collect()
}

#[test]
fn a_level_of_static_meshes_walks_the_column_once_and_reads_no_skeleton() {
    let mut scene = Scene::new();
    statics(&mut scene, 64);
    let (walks_before, reads_before) = (walks::of::<MeshComponent>(), probe::reads());
    assert!(scene.skinned_ids().is_empty());
    assert_eq!(
        walks::of::<MeshComponent>(),
        walks_before + 1,
        "one column walk"
    );
    assert!(scene.bone_subtrees().is_empty());
    scene.build_skin_palettes();
    assert_eq!(probe::reads(), reads_before, "no skeleton was read");
}

#[test]
fn rigs_among_static_meshes_come_back_in_insertion_order() {
    let mut scene = Scene::new();
    let early = statics(&mut scene, 5);
    let first = armature_owner(&mut scene);
    statics(&mut scene, 5);
    let second = armature_owner(&mut scene);
    scene.sync_skeletons();
    // A despawn swap-removes inside hecs's archetype, so its iteration order
    // drifts from insertion order: the sort on `Core::seq` must restore it.
    scene.destroy_entity(early[0]);
    let third = armature_owner(&mut scene);
    scene.sync_skeletons();
    assert_eq!(scene.skinned_ids(), vec![first, second, third]);
}

#[test]
fn a_mesh_is_seen_the_tick_it_becomes_skinned_or_stops_being() {
    let mut scene = Scene::new();
    let prop = static_mesh(&mut scene);
    let rig = armature_owner(&mut scene);
    scene.sync_skeletons();
    assert_eq!(scene.skinned_ids(), vec![rig]);
    // A model swap onto the prop: skinned once its skeleton is built.
    let skinned = scene.world.mesh(rig).unwrap().clone();
    scene.world.set_mesh(prop, Some(skinned));
    scene.world.mesh_mut(prop).unwrap().skeleton.bones.clear();
    scene.sync_skeletons();
    assert_eq!(scene.skinned_ids(), vec![prop, rig]);
    scene.world.set_mesh(rig, None);
    assert_eq!(scene.skinned_ids(), vec![prop]);
}

#[test]
fn a_rebuilt_palette_reuses_the_meshs_allocation() {
    let mut scene = Scene::new();
    statics(&mut scene, 3);
    let rig = armature_owner(&mut scene);
    scene.sync_skeletons();
    scene.build_skin_palettes();
    let held = scene.world.mesh(rig).unwrap().pose_palette.clone();
    let ptr = scene.world.mesh(rig).unwrap().pose_palette.as_ptr();
    let spine = scene.find_bone(rig, "spine").unwrap();
    scene.world.transform_mut(spine).unwrap().rotation = Quat::from_rotation_z(0.7);
    scene.build_skin_palettes();
    let mesh = scene.world.mesh(rig).unwrap();
    assert_eq!(mesh.pose_palette.as_ptr(), ptr, "no fresh palette per tick");
    assert_eq!(mesh.pose_palette.len(), held.len());
    assert_eq!(mesh.pose_palette[0], held[0], "the hips did not move");
    assert_ne!(mesh.pose_palette[2], held[2], "the head rode the spine");
}
