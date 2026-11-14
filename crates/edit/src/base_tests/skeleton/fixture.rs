//! The skeleton tests' fixtures that need the edit side: the asset layer's
//! two-joint `Joint0 → Joint1` rig instanced from its file as `Hero`, and the
//! save/load round trip. The synthetic rig and the joint helpers are the base's
//! test hooks (`rusty_scene::skeleton::rig`).

use glam::Vec3;

use crate::serialize::{apply_scene_data, to_scene_data};
use rusty_asset::AssetRef;
use rusty_scene::serialize::SceneData;
pub(super) use rusty_scene::skeleton::rig::{
    armature_owner, armature_skin, chain_skin, connected, joint_to, reimport_to, rope_to,
};
use rusty_scene::Scene;
pub(super) use rusty_test_support::logs::during as warnings_of;
use rusty_test_support::models::anim::animated_file;

/// Write the animated fixture under a per-test folder and return its
/// `path::Animated` reference.
pub(super) fn animated_model(tag: &str) -> AssetRef {
    let path = animated_file(&format!("rusty_453_{tag}"));
    AssetRef::new(
        &rusty_core::paths::to_slash(std::path::Path::new(&path)),
        "Animated",
    )
    .unwrap()
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
