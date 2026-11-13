//! Test fixtures for the skeleton module: an animated glTF on disk (the asset
//! layer's two-joint `Joint0 → Joint1` rig) and a synthetic skin whose root sits
//! under a rotated, scaled `Armature`; and the save/load and joint helpers the
//! skeleton tests share. The synthetic rig is `skeleton::rig`, a test hook other
//! crates' tests build on too.

use glam::Vec3;

pub(super) use super::rig::{armature_owner, armature_skin, chain_skin};
use crate::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::Scene;
use rusty_asset::{AssetRef, SkinData};
use rusty_components::JointComponent;
use rusty_test_support::models::anim::{animated_buffer, ANIMATED_GLTF};

/// Write the animated fixture under a per-test folder and return its
/// `path::Animated` reference.
pub(super) fn animated_model(tag: &str) -> AssetRef {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_453_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let path = dir.join("model.gltf");
    std::fs::write(&path, ANIMATED_GLTF).unwrap();
    AssetRef::new(&rusty_core::paths::to_slash(&path), "Animated").unwrap()
}

/// Instantiate the animated fixture as `Hero`, skeleton and all.
pub(crate) fn hero(scene: &mut Scene, tag: &str) -> u32 {
    crate::asset_instance::instantiate_asset(
        scene,
        &animated_model(tag).to_string_ref(),
        Some("Hero"),
        Vec3::ZERO,
    )
    .expect("the fixture instantiates")
}

/// Re-import `rig` to `skin` in place, as a re-written model does.
pub(super) fn reimport_to(scene: &mut Scene, rig: u32, skin: SkinData) {
    let parked = scene.take_skeleton(rig);
    scene.world.mesh_mut(rig).unwrap().skin = Some(skin);
    scene.restore_skeleton(rig, parked);
}

/// The engine warnings `f` logs on this thread.
pub(super) fn warnings_of(f: impl FnOnce()) -> Vec<String> {
    rusty_test_support::logs::init();
    rusty_test_support::logs::take(); // what earlier code on this thread logged
    f();
    rusty_test_support::logs::take()
}

/// Save `scene` to JSON (the on-disk path) and load it into a fresh scene.
pub(super) fn reload(scene: &Scene) -> Scene {
    reload_json(&serde_json::to_string(&to_scene_data(scene)).unwrap())
}

/// Load a saved scene's JSON into a fresh scene.
pub(super) fn reload_json(json: &str) -> Scene {
    let data: SceneData = serde_json::from_str(json).unwrap();
    let mut out = Scene::new();
    apply_scene_data(&mut out, data);
    out
}

/// A `Rope` entity jointed to `owner`'s bone `bone`.
pub(super) fn rope_to(scene: &mut Scene, owner: u32, bone: &str) -> u32 {
    let rope = scene.add_entity("Rope".to_string());
    let body = scene.find_bone(owner, bone);
    joint_to(scene, rope, body);
    rope
}

/// Give `id` a Joint connected to `body`.
pub(super) fn joint_to(scene: &mut Scene, id: u32, body: Option<u32>) {
    let joint = JointComponent {
        connected_body: body,
        ..JointComponent::default()
    };
    scene.world.set_joint(id, Some(joint));
}

/// The body `id`'s Joint is connected to.
pub(super) fn connected(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.joint(id).unwrap().connected_body
}
