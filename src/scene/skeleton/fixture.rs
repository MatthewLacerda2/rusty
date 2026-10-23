//! Test fixtures for the skeleton module: an animated glTF on disk (the asset
//! layer's two-joint `Joint0 → Joint1` rig) and a synthetic skin whose root sits
//! under a rotated, scaled `Armature`; and the save/load and joint helpers the
//! skeleton tests share.

use glam::{Mat4, Quat, Vec3};

use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::asset::{JointTransform, SkinData};
use crate::components::{JointComponent, MeshComponent};
use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// Write the animated fixture under a per-test folder and return its
/// `path::Animated` reference.
pub(super) fn animated_model(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_453_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let path = dir.join("model.gltf");
    std::fs::write(&path, ANIMATED_GLTF).unwrap();
    format!("{}::Animated", crate::core::paths::to_slash(&path))
}

/// Instantiate the animated fixture as `Hero`, skeleton and all.
pub(super) fn hero(scene: &mut Scene, tag: &str) -> u32 {
    crate::scene::asset_instance::instantiate_asset(
        scene,
        &animated_model(tag),
        Some("Hero"),
        Vec3::ZERO,
    )
    .expect("the fixture instantiates")
}

/// A three-joint chain (`hips → spine → head`, 1 unit apart on +Y) whose root
/// hangs under an `Armature` rotated 90° about X and scaled 0.5 — the offset a
/// Blender export puts between the mesh and the skeleton.
pub(super) fn armature_skin() -> SkinData {
    let armature = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.5),
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        Vec3::new(0.0, 0.0, 1.0),
    );
    let step = JointTransform {
        translation: Vec3::Y,
        ..JointTransform::default()
    };
    let local_bind = vec![JointTransform::default(), step, step];
    let parents = vec![None, Some(0), Some(1)];
    let mut bind_global: Vec<Mat4> = Vec::new();
    for (slot, local) in local_bind.iter().enumerate() {
        let parent = parents[slot].map_or(armature, |p: usize| bind_global[p]);
        bind_global.push(parent * local.matrix());
    }
    SkinData {
        inverse_bind: bind_global.iter().map(|g| g.inverse()).collect(),
        bind_global,
        local_bind,
        parents,
        joint_nodes: vec![1, 2, 3],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["hips".into(), "spine".into(), "head".into()],
    }
}

/// An entity carrying a skinned mesh of [`armature_skin`] (no file behind it).
pub(super) fn armature_owner(scene: &mut Scene) -> u32 {
    let skin = armature_skin();
    let id = scene.add_entity("Rig".to_string());
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            bind_palette: skin.bind_palette(),
            skin: Some(skin),
            is_dirty: Default::default(),
            ..MeshComponent::new("Asset", Vec::new(), Vec::new())
        }),
    );
    id
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
